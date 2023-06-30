use serde::{Deserialize, Serialize};
use serde_json::Number;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Debug;
use std::fs;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::path::Path;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use uuid::Uuid;
const CRLF: &str = "\r\n";

pub trait MethodCall: Send + Sync + 'static {
    fn call(&self, params: Vec<Value>) -> Option<Value>;
}
impl<F: Send + Sync + 'static> MethodCall for F
where
    F: Fn(Vec<Value>) -> Option<Value>,
{
    fn call(&self, params: Vec<Value>) -> Option<Value> {
        self(params)
    }
}
#[derive(Clone, Default)]
pub struct NetworkManager {
    sender: Option<Arc<ws::Sender>>,
    wait_senders: Arc<Mutex<HashMap<String, mpsc::Sender<MyMessage>>>>,
    pub map: HashMap<String, Arc<dyn MethodCall>>,
    pub str :Option<String>,
}
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct MyMessage {
    pub Typ: String,
    pub FuncName: Vec<String>,
    pub Params: Vec<Value>,
    pub Id: String,
    pub Data: Value,
}
pub trait Func: Debug + Sized {
    fn call<T>(&self, t: &Self, param: Vec<T>);
    fn all(&self) -> Vec<&Self>;
    fn create(&self, s: String) -> Self;
}

impl NetworkManager {
    pub fn new(m: HashMap<String, Arc<dyn MethodCall>>) ->  Self {
      
        NetworkManager {
            sender: None,
            wait_senders: Arc::new(Mutex::new(HashMap::new())),
            map:m,
            str:Some(String::from("test")),
        }
    }
    pub fn send(&self, m: MyMessage) -> Result<MyMessage, String> {
        if let Some(sender) = self.sender.as_ref() {
            let (s, r) = mpsc::channel::<MyMessage>();
            self.wait_senders
                .clone()
                .lock()
                .unwrap()
                .insert(m.Id.to_string(), s);
            sender
                .send(ws::Message::Text(serde_json::to_string(&m).unwrap()))
                .unwrap();
            let msg = r.recv().unwrap();
            self.wait_senders
                .clone()
                .lock()
                .unwrap()
                .remove(m.Id.as_str());
            return Ok(msg);
        }
        Ok(m)
    }
    pub fn bind2(&self) {
        for (k, _) in self.map.clone() {
            self.send(MyMessage {
                Id: Uuid::new_v4().to_string(),
                Typ: "bind".to_string(),
                FuncName: vec!["xz".to_string(), format!("{}", k)],
                ..Default::default()
            })
            .unwrap();
        }
    }
    pub fn run(sf:Arc<Mutex<Self>> , port: u16) {
        let (tx, reciver) = mpsc::channel();
        let sender = Arc::new(Mutex::new(tx));
        let wait_sender = sf.lock().unwrap().wait_senders.clone();
        let map = sf.lock().unwrap().map.clone();
        thread::spawn(move || {
            ws::listen(format!("127.0.0.1:{}", port), |out| {
                let out = Arc::new(out);
                sender.lock().unwrap().send(out.clone()).unwrap();
                let wait_sender = wait_sender.clone();
                let map = map.clone();
                move |msg| {
                    //  let out = out.clone();
                    if let ws::Message::Text(v) = msg {
                        let mm: MyMessage = serde_json::from_str(v.as_str()).unwrap();
                        if let Some(x) = wait_sender.clone().lock().unwrap().get(mm.Id.as_str()) {
                            x.send(mm.clone()).unwrap();
                        } else {
                            if mm.Typ == "call" {
                                if let Some(r) =
                                    map.get(mm.FuncName[1].as_str()).unwrap().call(mm.Params)
                                {
                                    if r != Value::Null {
                                        out.send(ws::Message::Text(
                                            serde_json::to_string(&MyMessage {
                                                Id: mm.Id,
                                                Data: r,
                                                ..Default::default()
                                            })
                                            .unwrap(),
                                        ))
                                        .unwrap();
                                    }
                                }
                            }
                        }
                    }
                    Ok(())
                }
            })
            .unwrap();
        });

        thread::spawn(move||{
            loop {
                let sender = reciver.recv().unwrap();
               {
                sf.lock().unwrap().sender = Some(sender);
                sf.lock().unwrap().str=Some(Uuid::new_v4().to_string());
               }
             
            }
        });
        
    }
}
#[derive(Copy, Clone)]
pub struct FileHttpManger {}
impl FileHttpManger {
    pub fn new() -> Self {
        FileHttpManger {}
    }
    pub fn run(self, dir: &'static str, port: u16) {
        thread::spawn(move || {
            let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();
            for l in listener.incoming() {
                thread::spawn(move || {
                    let mut stream = l.unwrap();
                    let mut line = String::new();
                    BufReader::new(&stream).read_line(&mut line).unwrap();
                    //let list = reader.lines().collect::<Vec<_>>();
                    // let line = list[0].as_ref().unwrap();
                    let route = line.split(" ").collect::<Vec<_>>()[1];
                    let path = route.trim_start_matches("/");
                    let p = Path::new(dir).join(path);
                    let contents;
                    let path_list = path.split("?").collect::<Vec<_>>();
                    println!("html={}", path_list[0]);
                    if path_list[0] == "DEFAULT.html" {
                        //let s = String::from(DEFAULT_HTML).replace("{url}", path_list[1].split("p=").collect::<Vec<_>>()[0]).replace("{port}", (port+1).to_string().as_str());
                        let s = String::from(DEFAULT_HTML)
                            .replace("{port}", (port + 1).to_string().as_str());
                        contents = s.as_bytes().to_vec();
                    } else {
                        contents = fs::read(p).unwrap_or_default();
                    }

                    let content_type = format!("Content-Type: text/html;charset=utf-8{}", CRLF);
                    let server = format!("Server: Rust{}", CRLF);
                    let content_length = format!("Content-Length: {}{}", contents.len(), CRLF);
                    let response = format!(
                        "{0}{1}{2}{3}{4}{5}",
                        format!("HTTP/1.1 {} OK{}", 200, CRLF),
                        server,
                        content_type,
                        content_length,
                        CRLF,
                        String::from_utf8(contents).unwrap()
                    );
                    stream.write(response.as_bytes()).unwrap();
                    stream.flush().unwrap();
                });
            }
        });
    }
}
const DEFAULT_HTML: &'static str = r#"
<html>
<title>%s</title>
<body style="margin:0;padding:0;background-color:#ffffff;">
<iframe src="" style="border:medium none;width:100vw;height:100vh;" frameborder="0" id="iframe" style="margin:0;padding:0;"></iframe>
</body>
</html>
<script>
    var wsServer = 'ws://127.0.0.1:{port}/ws';
    var websocket;
    var requestMap = {};
    var iframe;
    window.onload = function () {
        iframe = document.getElementById("iframe");
        startWebsocket();
    }
    function startWebsocket() {
        websocket = new WebSocket(wsServer);
        websocket.onopen = function (evt) {
            console.log(evt)
        };
        websocket.onclose = function (evt) {
            setTimeout(() => {
                startWebsocket();
            }, 1000)
        };
        websocket.onmessage = function (evt) {
            let msg = JSON.parse(evt.data);
            if (msg.Typ == "nav") {
                let r = eval(msg.Data);
                setTimeout(() => {
                    msg.Data = r;
                    websocket.send2(msg);
                }, 1000)

            } else if (msg.Typ == "eval") {
                let r = iframe.contentWindow.eval(msg.Data);
                if(!r)r="";
                msg["Data"] = r;
                websocket.send2(msg);
            } else if (msg.Typ == "bind") {
                let fs = msg.FuncName;
                if (fs.length != 2) {
                    return;
                }
                if(!iframe.contentWindow[fs[0]])iframe.contentWindow[fs[0]] = {};
                iframe.contentWindow[fs[0]][fs[1]] = function () {
                    msg.FuncName=fs;
                    msg.Typ="call";
                    msg.Id = Date.now() + "";
                    msg.Params = [];
                    //let data = {FuncName: fs, Params: []};
                    let data = msg;
                    let callback = null;
                    if (arguments.length > 0) {
                        for (let k in arguments) {
                            if (k == arguments.length - 1 && typeof arguments[k] === "function") {
                                callback = arguments[k];
                            } else {
                                data.Params.push(arguments[k])
                            }

                        }
                    }
                    websocket.send2(data, callback)
                }
                websocket.send2(msg);
            } else if (msg.Typ == "RemoveBind") {
                let fs = msg.FuncName;
                if (fs.length != 2) {
                    return;
                }
                iframe.contentWindow[fs[0]] = {};
            } else if (requestMap[msg.Id]) {
                requestMap[msg.Id](msg.Data)
            }
            
        };

        websocket.onerror = function (evt, e) {
            console.log('Error occured: ' + evt.data);
        };
        websocket.send2 = function (data, f) {
            console.log("send2", data)
            if (!data.Id) data.Id = Date.now() + "";
            if (f) requestMap[data.Id] = f;
            websocket.send(JSON.stringify(data));
        }
    }

</script>
"#;
