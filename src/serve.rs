use std::io;
use std::path::{Path, PathBuf};
use std::process;
use std::env;

#[macro_use]
use rocket;
use rocket::response::{Response, NamedFile, Responder};
use rocket::http::Status;
use rocket::Request;

#[get("/")]
fn index() -> io::Result<NamedFile> {
    NamedFile::open("index.html")
}

#[get("/<file..>")]
fn files(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(file).ok()
}

enum NotFoundResp {
    File(NamedFile),
    Text(String)   ,
}

impl Responder<'static> for NotFoundResp {
    fn respond_to(self, req: &Request) -> Result<Response<'static>, Status> {
        match self {
             NotFoundResp::File(f) => f.respond_to(req),
             NotFoundResp::Text(t) => t.respond_to(req),
        }
    }
}


#[error(404)]
fn not_found(req : &Request) -> NotFoundResp {
      let mut potential = PathBuf::from(req.uri().as_str());
      potential.set_extension("html");
      potential = potential.strip_prefix(Path::new("/")).unwrap().to_path_buf();
      if potential.exists() {
          if let Some(file) = NamedFile::open(&potential).ok() {
              return NotFoundResp::File(file)
          }
      }
      else {
          let err_path = PathBuf::from("404.html");
          if err_path.exists() {
              if let Some(errfile) = NamedFile::open(&err_path).ok() {
                  return NotFoundResp::File(errfile)
              }
          }
      }

      NotFoundResp::Text(String::from("404: Not Found"))
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
