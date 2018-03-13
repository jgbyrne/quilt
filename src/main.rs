extern crate walkdir;

use std::convert;
use std::env;
use std::process;
use std::path::PathBuf;
use std::ffi::OsStr;
use std::collections::HashMap;

fn quilt_err<'a>(err: &'a str) -> ! {
    eprintln!("Error: {}", err);
    process::exit(1);
}

fn quilt_assert<'a>(cond: bool, err: &'a str) {
    if !cond {
        quilt_err(err);    
    }
}

struct QuiltError {
     source : String,
     message: String,
}

impl convert::From<walkdir::Error> for QuiltError{
    fn from(err: walkdir::Error) -> Self {
        QuiltError {source : "WalkDir".to_owned(),
                    message: format!("{}", err)}
    }
}

#[derive(Debug)]
struct Page {
    pub name       : String,
    pub section_id : usize ,
    pub has_toml   : bool  ,
    pub has_md   : bool  ,
}

#[derive(Debug)]
struct Site {
    site_dir   : PathBuf,
    static_dir : Option<PathBuf>,
    sections   : Vec<PathBuf>,
    pages      : HashMap<PathBuf, Page>,    
}

#[derive(Debug)]
struct Job<'args> {
    from_path : &'args str   ,
    to_path   : &'args str   ,
    site      : Option<Site>,
}

impl<'args> Job<'args> {
    fn compose(&mut self) -> Result<(), QuiltError> {

        let mut pages: HashMap<PathBuf, Page> = HashMap::new(); 

        let mut cursec_path = PathBuf::from("site");
        let mut cursec_id   = 0;

        let mut has_static: bool = false;
        let mut has_site  : bool = false;
        let mut sections  : Vec<PathBuf>    = vec![cursec_path.clone()];

        let static_dir = PathBuf::from(self.from_path).join("static");
        let site_dir   = PathBuf::from(self.from_path).join("site");
        let from_dir   = PathBuf::from(self.from_path);

        let mut await_context = false;
        let mut mapping = false;
        
        for entry in walkdir::WalkDir::new(self.from_path) {
            let entry = entry?;

            println!("{}", entry.path().display());

            if entry.path() == from_dir {
                continue;
            }

            let parent = entry.path().parent().unwrap();
            
            if parent  == from_dir && entry.file_type().is_dir() {
                await_context = false;
                if entry.path() == static_dir {
                    has_static = true;
                    await_context = true;
                    continue
                }

                if entry.path() == site_dir {
                    has_site = true;
                    mapping = true;
                    continue
                }
            }
            else {
                if await_context {
                    continue;
                }
            }

            if mapping {
                if entry.file_type().is_dir() {
                    let path    = entry.path();
                    cursec_path = (*path.strip_prefix(&from_dir).unwrap()).to_path_buf();
                    cursec_id   = sections.len();
                    sections.push(cursec_path.clone());
                    continue
                }
                else if let Some(name) = entry.path().file_stem() {
                    let parent   = entry.path().parent().unwrap(); 
                    let sec_path = (*parent.strip_prefix(&from_dir).unwrap()).to_path_buf();
                    if sec_path != cursec_path {
                        cursec_path = sec_path;
                        for (i, pb) in sections.iter().enumerate().rev() {
                            if *pb == cursec_path {
                                 cursec_id = i;
                            }
                        }
                    }

                    let ext = entry.path().extension().unwrap_or(OsStr::new(""));
                    
                    if ext == "" {
                        continue
                    }

                    let page_path = cursec_path.join(name);
                    let mut new_page : Option<Page>  = None;
                    match pages.get_mut(&page_path) {
                        Some(page) => {
                                       if page.has_md {
                                           quilt_assert(ext == "toml",
                                                        &format!("Unexpected File: {:?}", entry.path()));
                                           
                                           page.has_toml = true;

                                       }
                                       else {
                                           quilt_assert(ext == "md",
                                                        &format!("Unexpected File: {:?}", entry.path()));
                                           
                                           page.has_md = true;
                                       }
                                      },

                        None       => {
                                       new_page = Some(Page {name: name.to_str().unwrap().to_owned(),
                                                             section_id: cursec_id,
                                                             has_md  : (ext == "md")   ,
                                                             has_toml: (ext == "toml") ,    });
                                      }
                    }
                    
                    if let Some(new) = new_page {
                        pages.insert(page_path, new);
                    }
                }
            }
        }

        quilt_assert(has_site, "/site directory not found");

        let static_opt = if has_static { Some(static_dir) } else { None };

        let site = Site {site_dir : site_dir, static_dir : static_opt, sections : sections, pages: pages};
        self.site = Some(site);

        Ok(())
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        quilt_err("Not enough arguments.");
    }

    let from: &str = &args[1];
    let to  : &str = &args[2];
    
    let mut job = Job {from_path: from, to_path: to, site: None};
    
    match job.compose() {
        Ok(_)  => () ,
        Err(e) => quilt_err(&format!("[{}] {}", e.source, e.message))
    }

    println!("{:#?}", job);

}
