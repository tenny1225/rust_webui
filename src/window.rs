use serde_json::Value;
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::path::Path;
use std::pin::Pin;
use std::process::Command;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::{env, io, thread, time};
use uuid::Uuid;

use crate::net::{FileHttpManger, Func, MethodCall, MyMessage, NetworkManager};
trait Callback: Send + Sync + 'static {
    fn call(&self, c: &Chrome);
}
impl<F: Send + Sync + 'static> Callback for F
where
    F: Fn(&Chrome),
{
    fn call(&self, c: &Chrome) {
        self(c);
    }
}

pub struct Chrome {
    pub params: Vec<String>,
    pub local_path: Option<&'static str>,
    pub network_manager: Option<Arc<Mutex<NetworkManager>>>,
    pub call_back: Option<Arc<dyn Callback>>,
    pub map: HashMap<String, Arc<dyn MethodCall>>,
    port: u16,
}
impl Chrome {
    pub fn new() -> Chrome {
        Chrome {
            params: vec![],
            local_path: None,
            network_manager: None,
            call_back: None,
            map: HashMap::new(),
            port: 0,
        }
    }
    pub fn ui<F>(mut self, f: F) -> Self
    where
        F: Callback,
    {
        self.call_back = Some(Arc::new(f));
        return self;
    }
    pub fn size(mut self, w: usize, h: usize) -> Self {
        self.params.push(format!("--window-size={},{}", w, h));
        return self;
    }
    pub fn pos(mut self, x: usize, y: usize) -> Self {
        self.params.push(format!("--window-position={},{}", x, y));
        return self;
    }
    fn url(&mut self, url: &str) {
        self.params.push(format!("--app={}", url));
    }
    pub fn local(&mut self, path: &'static str) {
        self.local_path = Some(path);
    }

    pub fn bind<T: MethodCall>(mut self, name: &str, f: T) -> Self {
        self.map.insert(name.to_string(), Arc::new(f));
        self
    }
    pub fn eval(&self, js: String)->Result<Value,String> {
       let r =  self.network_manager
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .send(MyMessage {
                Id: Uuid::new_v4().to_string(),
                Typ: "eval".to_string(),
                Data: Value::String(js),
                ..Default::default()
            })?;
       Ok(r.Data)

    }
    pub fn nav(&self, url: String) {
        let mut src = url;
        if !src.starts_with("http") {
            src = format!("http://127.0.0.1:{}/{}", self.port, src);
        }
        let nm = self.network_manager
            .as_ref()
            .unwrap()
            .lock()
            .unwrap();
            nm.send(MyMessage {
                Id: Uuid::new_v4().to_string(),
                Typ: "nav".to_string(),
                Data: Value::String(format!(
                    r#"document.getElementById("iframe").src="{}""#,
                    src
                )),
                ..Default::default()
            })
            .unwrap();
        nm.bind2();
    }
    pub fn run(mut self, httpport: u16) {
        let user_dir = if cfg!(target_os = "windows") {
            "C:\tmp".to_string()
        } else {
            "/tmp".to_string()
        };
        self.port = httpport;
        self.params.push(format!("--user-data-dir={}", user_dir));
        // self.params.push(format!("--window-size={},{}",self.width,self.height));
        self.params.push("--remote-debugging-port=0".to_string());
        self.url(format!("http://127.0.0.1:{}/DEFAULT.html", httpport).as_str());
        let params = self.params.clone();
        let local_path = self.local_path.clone();
        let network = Arc::new(Mutex::new(NetworkManager::new(self.map.clone())));

        NetworkManager::run(network.clone(), httpport + 1);
        self.network_manager = Some(network.clone());

        let mut dir = "./";
        if let Some(v) = local_path {
            dir = &v[..];
        }
        FileHttpManger::new().run(dir, httpport);
        let callback = self.call_back.as_ref().unwrap().clone();
        // let chome = Arc::new(self);
        thread::spawn(move || {
            let ten_millis = time::Duration::from_millis(3000);
            sleep(ten_millis);
            callback.call(&self);
        });

        let mut params = params.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        params.insert(0, get_chrome_path().unwrap());
        let default_params = DEFAULT_PARAMS
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>();
        params = [params, default_params].concat();
        let arg = params.join(" ");

        if cfg!(target_os = "windows") {
            Command::new("cmd").arg("/C").arg(arg).output().unwrap();
        } else {
            Command::new("sh").arg("-c").arg(arg).output().unwrap();
        }

        // task.join().unwrap();
    }
}
const DEFAULT_PARAMS: &'static [&'static str] = &[
    "--disable-renderer-backgrounding",
    "--disable-sync",
    "--disable-translate",
    "--disable-windows10-custom-titlebar",
    "--metrics-recording-only",
    "--no-first-run",
    "--no-default-browser-check",
    "--safebrowsing-disable-auto-update",
    //"--enable-automation",
    "--password-store=basic",
    "--use-mock-keychain",
    "--disable-background-networking",
    "--disable-background-timer-throttling",
    "--disable-backgrounding-occluded-windows",
    "--disable-breakpad",
    "--disable-client-side-phishing-detection",
    "--disable-default-apps",
    "--disable-dev-shm-usage",
    "--disable-infobars",
    "--disable-extensions",
    "--disable-features=site-per-process",
    "--disable-hang-monitor",
    "--disable-ipc-flooding-protection",
    "--disable-popup-blocking",
    "--disable-prompt-on-repost",
    "--ignore-certificate-errors",
    "--allow-running-insecure-content",
    "--disable-web-security",
];
fn get_chrome_path() -> Result<String, String> {
    let paths: Vec<String>;
    if cfg!(target_os = "darwin") {
        paths = vec![
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".to_string(),
            "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary"
                .to_string(),
            "/Applications/Chromium.app/Contents/MacOS/Chromium".to_string(),
            "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge".to_string(),
            "/usr/bin/google-chrome-stable".to_string(),
            "/usr/bin/google-chrome".to_string(),
            "/usr/bin/chromium".to_string(),
            "/usr/bin/chromium-browser".to_string(),
        ];
    } else if cfg!(target_os = "windows") {
        let env_vars: HashMap<String, String> = env::vars().collect();
        paths = vec![
            format!(
                "{}/Google/Chrome/Application/chrome.exe",
                env_vars.get("LocalAppData").unwrap_or(&String::from(""))
            ),
            format!(
                "{}/Google/Chrome/Application/chrome.exe",
                env_vars.get("ProgramFiles").unwrap_or(&String::from(""))
            ),
            format!(
                "{}/Google/Chrome/Application/chrome.exe",
                env_vars
                    .get("ProgramFiles(x86)")
                    .unwrap_or(&String::from(""))
            ),
            format!(
                "{}/Chromium/Application/chrome.exe",
                env_vars.get("LocalAppData").unwrap_or(&String::from(""))
            ),
            format!(
                "{}/Chromium/Application/chrome.exe",
                env_vars.get("ProgramFiles").unwrap_or(&String::from(""))
            ),
            format!(
                "{}/Chromium/Application/chrome.exe",
                env_vars
                    .get("ProgramFiles(x86)")
                    .unwrap_or(&String::from(""))
            ),
            format!(
                "{}/Microsoft/Edge/Application/msedge.exe",
                env_vars
                    .get("ProgramFiles(x86)")
                    .unwrap_or(&String::from(""))
            ),
            format!(
                "{}/Microsoft/Edge/Application/msedge.exe",
                env_vars.get("ProgramFiles").unwrap_or(&String::from(""))
            ),
        ];
    } else {
        paths = vec![
            "/usr/bin/google-chrome-stable".to_string(),
            "/usr/bin/google-chrome".to_string(),
            "/usr/bin/chromium".to_string(),
            "/usr/bin/chromium-browser".to_string(),
            "/snap/bin/chromium".to_string(),
        ];
    }
    for p in paths {
        if Path::new(p.as_str()).exists() {
            return Ok(p);
        }
    }
    Err(String::from("chrome not exist!"))
}
