# Download only an explicit returned URL. Never emit URLs, bodies or credentials.
import json
import socket
import sys
import urllib.error
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET


class ArtifactError(Exception):
    pass


class Redirects(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, msg, headers, newurl):
        if urllib.parse.urlparse(newurl).scheme != 'https':
            raise ArtifactError('unsafe_redirect')
        return super().redirect_request(request, fp, code, msg, headers, newurl)


def download(req):
    url = req['url']
    parsed = urllib.parse.urlparse(url)
    blob = parsed.path.removeprefix('/blob/')
    local_blob = (parsed.scheme == 'http' and parsed.hostname == '127.0.0.1'
                  and parsed.path.startswith('/blob/') and len(blob) == 64
                  and all(c in '0123456789abcdefABCDEF' for c in blob)
                  and not parsed.query and not parsed.fragment)
    if any(ord(c) < 32 or ord(c) == 127 for c in url) or not (
            parsed.scheme == 'https' or local_blob):
        raise ArtifactError('invalid_url')
    opener = urllib.request.build_opener(Redirects)
    request = urllib.request.Request(url, headers={'User-Agent': 'StepX Desktop'})
    with opener.open(request, timeout=60) as response:
        size = 0
        first = b''
        mime = response.headers.get_content_type()
        with open(req['output'], 'wb') as out:
            while True:
                part = response.read(65536)
                if not part:
                    break
                if not first:
                    first = part[:32]
                size += len(part)
                if size > 32 * 1024 * 1024:
                    raise ArtifactError('too_large')
                out.write(part)
        if not size:
            raise ArtifactError('empty')
        if req.get('image'):
            valid = (first.startswith(b'\x89PNG\r\n\x1a\n')
                     or first.startswith(b'\xff\xd8\xff')
                     or first.startswith((b'GIF87a', b'GIF89a'))
                     or (first.startswith(b'RIFF') and first[8:12] == b'WEBP'))
            if not valid:
                raise ArtifactError('invalid_image')
        return {'ok': True, 'bytes': size, 'mime': mime}


def http_failure(error):
    # Inspect a bounded service error only to classify it. Never return its text:
    # object storage errors can echo signed URLs and credential identifiers.
    raw = b''
    try:
        raw = error.read(4096)
        body = json.loads(raw)
        if not isinstance(body, dict):
            body = {}
    except Exception:
        body = {}
        # S3/OSS services also return bounded XML error documents. Only retain
        # the two classification fields, never the signed request or service body.
        try:
            root = ET.fromstring(raw)
            for element in root.iter():
                key = element.tag.rsplit('}', 1)[-1]
                if key in ('Code', 'Message'):
                    body[key] = element.text or ''
        except (ET.ParseError, ValueError):
            pass
    message = str(body.get('Message', body.get('message', ''))).lower()
    code = str(body.get('Code', body.get('code', ''))).lower()
    expired = (code in ('expiredtoken', 'requestexpired')
               or 'expired' in message or 'expiration' in message)
    reason = 'http'
    if error.code in (401, 403):
        reason = 'expired' if expired else 'access_denied'
    elif error.code in (404, 410):
        reason = 'not_found'
    elif error.code == 429:
        reason = 'rate_limited'
    elif error.code >= 500:
        reason = 'service_unavailable'
    return {'ok': False, 'error': reason, 'http_status': error.code}


def main():
    try:
        result = download(json.load(sys.stdin))
    except urllib.error.HTTPError as error:
        result = http_failure(error)
    except (TimeoutError, socket.timeout):
        result = {'ok': False, 'error': 'timeout'}
    except urllib.error.URLError as error:
        result = {'ok': False, 'error': 'timeout' if isinstance(
            error.reason, (TimeoutError, socket.timeout)) else 'network'}
    except ArtifactError as error:
        result = {'ok': False, 'error': str(error)}
    except Exception:
        result = {'ok': False, 'error': 'download_failed'}
    print(json.dumps(result))
    return 0 if result['ok'] else 1


if __name__ == '__main__':
    sys.exit(main())
