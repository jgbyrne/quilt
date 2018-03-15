use std::io;
use std::path::{Path, PathBuf};
use std::process;
use std::env;

#[macro_use]
use rocket;
use rocket::response::NamedFile;
use rocket::Request;

#[get("/")]
fn index() -> io::Result<NamedFile> {
    NamedFile::open("index.html")
}

#[get("/<file..>")]
fn files(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(file).ok()
}

#[error(code = 404)]
fn not_found(req : &Request) -> Option<NamedFile> {
      let mut potential = PathBuf::from(req.uri().as_str());
      potential.set_extension("html");
      potential = potential.strip_prefix(Path::new("/")).unwrap().to_path_buf();
      println!("{:?}", potential);
      if potential.exists() {
          NamedFile::open(&potential).ok()
      }
      else {
          let err_path = PathBuf::from("404.html");
          if err_path.exists() {
              NamedFile::open(err_path).ok()
          }
          else {
              None
          }
      }
}

fn rocket() -> rocket::Rocket {
    rocket::ignite()
        .catch(errors![not_found])
        .mount("/", routes![index, files])
}

pub fn serve<P: AsRef<Path>>(serve_dir: P) -> ! {
    env::set_current_dir(serve_dir);
    rocket().launch();
    process::exit(0);
}
