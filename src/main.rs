mod net;
mod window;
use std::thread::sleep;
use std::time;
use std::{any::Any, thread::Thread};
use std::fmt::Debug;
use std::sync::Arc;

use serde_json::{Value, Number};

fn main() {
        window::Chrome::new()
        .size(300, 500)
        .pos(200, 200)
        .bind("click", |p:Vec<Value>|{

            let mut count = 0;
            for i in p{
                count+= i.as_i64().unwrap();
            }
            Some(Value::Number(Number::from(count)))
        })
        .ui(|x:&window::Chrome|{
            x.nav("xz.html".to_string());
            let ten_millis = time::Duration::from_millis(5000);
            sleep(ten_millis);
            x.eval("alert('xz')".to_string());
            
        }).run(9000);

}