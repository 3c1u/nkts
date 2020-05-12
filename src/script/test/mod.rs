use crate::script::vm::Vm;
use std::io::Cursor;
use encoding_rs::SHIFT_JIS;

#[test]
fn parse_script() {
    let script = include_bytes!("0X_RT_XX.txt");
    let (script, _, _) = SHIFT_JIS.decode(script);
    let script = Cursor::new(&*script);

    let mut script = Vm::new(script);

    while script.load_command_until_wait().unwrap() {
        // fuck
        println!("wait");
    }
}
