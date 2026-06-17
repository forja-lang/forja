use forja::jit::NativeJIT;
use forja::bytecode::Opcode::*;
use std::io::Write;

fn main() {
    println!("Starting...");
    let mut jit = NativeJIT::new();
    println!("JIT created");
    std::io::stdout().flush().ok();

    let bc = vec![PushEntero(42), Halt];
    println!("Compiling {} bytes...", bc.len());
    std::io::stdout().flush().ok();

    match jit.compile("t", &bc) {
        Ok(ptr) => {
            println!("Compiled OK at {:p}", ptr);
            std::io::stdout().flush().ok();
            let mut vars = vec![0i64; 256];
            let mut output = Vec::new();
            println!("Executing...");
            std::io::stdout().flush().ok();
            let r = unsafe { jit.execute("t", &mut vars, &mut output) };
            println!("Result: {:?}", r);
        }
        Err(e) => println!("Compile error: {}", e),
    }
}
