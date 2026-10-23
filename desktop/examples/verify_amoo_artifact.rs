//! Read-only verification of an attachment already returned by the phone.
//! Never logs the URL, query string, blob or account credentials.
use anyhow::{Context, Result};
use stepx_companion::{adb::Adb, amoo::AgentBus, amoo_media, device::Device};

fn main() -> Result<()> {
    let serial = std::env::args()
        .nth(1)
        .context("explicit device required")?;
    let expected = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "starry-night-illustration.png".into());
    let adb = Adb::discover()?;
    let device = Device::inspect(&adb, &serial)?;
    let mut bus = AgentBus::connect(&adb, &device)?;
    let data = bus.snapshot(None)?;
    let attachment = data
        .sessions
        .iter()
        .flat_map(|s| s.messages.iter())
        .flat_map(|m| m.attachments.iter())
        .find(|a| a.name == expected)
        .context("requested attachment absent in latest session")?;
    println!(
        "attachment_found=true kind={} mime={} name={} reported_bytes={:?}",
        attachment.kind, attachment.mime, attachment.name, attachment.size_bytes
    );
    let private = tempfile::tempdir()?;
    let metadata = private.path().join("attachment.json");
    std::fs::write(&metadata, serde_json::to_vec(attachment)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&metadata, std::fs::Permissions::from_mode(0o600))?;
    }
    let script = r#"
import json,sys,urllib.parse,urllib.request,urllib.error,datetime
a=json.load(open(sys.argv[1])); u=a.get('url') or a.get('blob')
p=urllib.parse.urlparse(u)
query=urllib.parse.parse_qs(p.query)
print(json.dumps({'scheme':p.scheme,'host':p.hostname,'query_keys':sorted(query.keys()),'literal_amp_escape':'\\u0026' in u,'html_amp_escape':'&amp;' in u,'signed_date':query.get('X-Tos-Date'),'expires_seconds':query.get('X-Tos-Expires'),'now_utc':datetime.datetime.now(datetime.timezone.utc).isoformat()}))
try:
 with urllib.request.urlopen(urllib.request.Request(u,headers={'User-Agent':'StepX Desktop'}),timeout=30) as r:
  chunk=r.read(32)
  print(json.dumps({'http_status':r.status,'content_type':r.headers.get_content_type(),'content_length':r.headers.get('Content-Length'),'magic_hex':chunk[:12].hex()}))
except urllib.error.HTTPError as e:
 try: body=json.loads(e.read(4096))
 except Exception: body={}
 message=str(body.get('Message',body.get('message',''))).lower()
 code=str(body.get('Code',body.get('code','')))
 print(json.dumps({'http_status':e.code,'content_type':e.headers.get_content_type(),'error_type':'HTTPError','service_error_code':code if code.isalnum() else 'redacted','reports_expiry':'expir' in message,'reports_signature':'signature' in message,'server_date':e.headers.get('Date')}))
except urllib.error.URLError as e:
 print(json.dumps({'error_type':'URLError','reason_type':type(e.reason).__name__}))
except Exception as e:
 print(json.dumps({'error_type':type(e).__name__}))
"#;
    let probe = std::process::Command::new("python3")
        .arg("-c")
        .arg(script)
        .arg(&metadata)
        .output()?;
    print!("{}", String::from_utf8_lossy(&probe.stdout));
    let output = private.path().join("download.png");
    match amoo_media::download_artifact(&adb, &device, attachment, &output) {
        Ok(file) => {
            let image = image::open(&file.path)?;
            println!(
                "download_verified=true bytes={} width={} height={}",
                file.bytes,
                image.width(),
                image.height()
            );
        }
        Err(e) => println!("download_verified=false error={e:#}"),
    }
    Ok(())
}
