# Bounded HTTPS-only metadata fetch. No executable or package is downloaded.
import json, sys, urllib.request, urllib.parse, urllib.error

def validate(url):
    parsed = urllib.parse.urlsplit(url)
    if parsed.scheme != 'https' or not parsed.hostname or parsed.username or parsed.password or parsed.fragment or any(c.isspace() or ord(c) < 32 for c in url) or '\\' in url:
        raise ValueError('INVALID_URL')
    return url

class Redirects(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        validate(newurl)
        return super().redirect_request(req, fp, code, msg, headers, newurl)

try:
    url = validate(json.load(sys.stdin)['url'])
    opener = urllib.request.build_opener(Redirects())
    with opener.open(urllib.request.Request(url, headers={'User-Agent':'StepX-Desktop-Update-Check/1', 'Accept':'application/json'}), timeout=15) as response:
        kind = response.headers.get_content_type()
        if kind not in ('application/json', 'text/plain', 'application/octet-stream'):
            raise ValueError('INVALID_CONTENT')
        data = response.read(512 * 1024 + 1)
        if len(data) > 512 * 1024:
            raise ValueError('TOO_LARGE')
        sys.stdout.buffer.write(data)
except urllib.error.HTTPError as error:
    sys.stderr.write('HTTP' + str(error.code)); sys.exit(1)
except ValueError as error:
    sys.stderr.write(str(error) if str(error) in ('INVALID_URL','INVALID_CONTENT','TOO_LARGE') else 'NETWORK'); sys.exit(1)
except Exception:
    sys.stderr.write('NETWORK'); sys.exit(1)
