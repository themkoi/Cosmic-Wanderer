use std::io::{self, Write};
use std::os::unix::net::UnixStream;

fn main() -> io::Result<()> {
    let mut stream = UnixStream::connect("/tmp/comsic-wanderer.sock")?;
    stream.write_all(&[0])?;
    Ok(())
}
