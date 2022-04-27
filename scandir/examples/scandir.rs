use std::env;
use std::io::Error;
use std::result::Result;

use scandir::{self, ReturnType, Scandir};

fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().collect();
    let mut instance = Scandir::new(&args[1])?;
    if args.len() > 2 {
        instance = instance.return_type(ReturnType::Ext);
    }
    instance.collect()?;
    println!("{}", &format!("{:#?}", instance.results(true))[..2000]);
    println!("{:?}", instance.finished());
    println!("{:?}", instance.has_entries());
    println!("{:?}", instance.has_errors());
    println!("{:?}", instance.duration());
    Ok(())
}
