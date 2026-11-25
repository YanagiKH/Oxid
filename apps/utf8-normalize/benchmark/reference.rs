use std::io::{self, Read, Write};
use std::process::ExitCode;

fn normalize(input: &mut impl Read, output: &mut impl Write) -> u8 {
    let mut buffer = [0; 4096];
    let mut carry = 0;
    let mut previous_cr = false;
    loop {
        let count = match input.read(&mut buffer[carry..]) {
            Ok(0) => return if carry == 0 { 0 } else { 65 },
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return 74,
        };
        let extent = carry + count;
        let prefix = match std::str::from_utf8(&buffer[..extent]) {
            Ok(_) => extent,
            Err(error) if error.error_len().is_none() => error.valid_up_to(),
            Err(_) => return 65,
        };
        if prefix > 0 {
            let initial = usize::from(previous_cr && buffer[0] == b'\n');
            previous_cr = buffer[prefix - 1] == b'\r';
            let mut offset = initial;
            let mut written = initial;
            while offset < prefix {
                let byte = buffer[offset];
                buffer[written] = if byte == b'\r' { b'\n' } else { byte };
                written += 1;
                offset += 1;
                if byte == b'\r' && offset < prefix && buffer[offset] == b'\n' {
                    offset += 1;
                }
            }
            if output.write_all(&buffer[initial..written]).is_err() {
                return 74;
            }
        }
        carry = extent - prefix;
        buffer.copy_within(prefix..extent, 0);
    }
}

fn main() -> ExitCode {
    let mut stdin = io::stdin().lock();
    let mut stdout = io::stdout().lock();
    let mut status = normalize(&mut stdin, &mut stdout);
    if stdout.flush().is_err() && status == 0 {
        status = 74;
    }
    ExitCode::from(status)
}
