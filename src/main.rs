extern crate walkdir;
extern crate pulldown_cmark;
extern crate time;

use std::convert;
use std::env;
use std::process;
use std::fs;
use std::io::{Read, Write};
use std::path::{PathBuf, Component};
use std::ffi::OsStr;
use std::collections::HashMap;
use pulldown_cmark as markdown;

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

impl convert::From<std::io::Error> for QuiltError{
    fn from(err: std::io::Error) -> Self {
        QuiltError {source : "IO".to_owned(),
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

    fn build(&mut self) -> Result<(), QuiltError> {
        quilt_assert(self.site.is_some(), "Internal Error - this should not happen.");

        let build_dir = PathBuf::from(self.to_path);

        if build_dir.exists() {
            let quiltf = build_dir.join("_quilt");
            
            if quiltf.exists() {
                let mut qf_buf = String::new();
                
                {
                    let mut qf     = fs::File::open(&quiltf)?;
                    qf.read_to_string(&mut qf_buf);
                }

                for l in qf_buf.lines() {
                    if l.starts_with("!") {
                        let mut l = l.to_owned();
                        l.remove(0);
                        let delpath = build_dir.join(l);
                        if delpath.is_dir() {
                            fs::remove_dir_all(delpath)?;
                        }
                        else {
                            quilt_err("Bad _quilt file: ! precedes a file name.")
                        }
                    } 
                    else if !l.starts_with("#") {
                        let delpath = build_dir.join(l);
                        if delpath.is_dir() {
                            fs::remove_dir(delpath)?;
                        }
                        else {
                            fs::remove_file(delpath)?;
                        }
                    }
                }
                fs::remove_file(quiltf)?;
            }



            else {
                let prefix = build_dir.components().last().unwrap().as_os_str().to_str().unwrap();
                let move_name = format!("{}-old-{}", prefix, time::now_utc().to_timespec().sec);
                fs::rename(build_dir.clone(), build_dir.parent().unwrap().join(move_name));
            }
        }

        if let Some(ref site) = self.site {
            let mut qf_lines : Vec<String> = vec![];

            for section in &site.sections {
                let adj = section.strip_prefix("site").unwrap();
                fs::create_dir_all(build_dir.join(adj))?;

                if let Some(Component::Normal(ref s)) = adj.components().next() {
                    qf_lines.push(s.to_str().unwrap().to_owned());
                }
            }
            
            for (path, page) in &site.pages {
                 println!("{:?} {:?}", path, page);

                 if !page.has_md {
                     println!("Page {} ({}) does not have an associated markdown file - skipping.", page.name, path.display());
                     continue;
                 }

                 let adjusted_path = path.strip_prefix("site").unwrap().to_path_buf();

                 let mut md_path = site.site_dir.join(&adjusted_path);
                 md_path.set_extension("md");
                 
                 let mut page_md = fs::File::open(md_path)?;
                 
                 let mut md_buf = String::new();
                 page_md.read_to_string(&mut md_buf);

                 let parser = markdown::Parser::new(&md_buf);
            
                 let mut html_buf = String::new();
                 markdown::html::push_html(&mut html_buf, parser);

                 let mut html_path = build_dir.join(adjusted_path);
                 html_path.set_extension("html");

                 qf_lines.push(html_path.strip_prefix(&build_dir)
                                        .unwrap()
                                        .to_str()
                                        .unwrap()
                                        .to_owned());

                 println!("{}", html_path.display());
                 let mut page_html = fs::File::create(html_path)?;
                 page_html.write_all(&html_buf.as_bytes())?;
            }

            if let Some(ref static_dir) = site.static_dir {
                for entry in walkdir::WalkDir::new(static_dir) {
                    let entry = entry?;

                    if entry.path().is_file() {
                        let thread = entry.path().strip_prefix(static_dir).unwrap();
                        let dirs   = thread.parent().unwrap();
                        let linked = build_dir.join("static").join(dirs);
                        fs::create_dir_all(&linked)?;

                        let fpath = linked.join(entry.path().file_name().unwrap());
                        fs::copy(entry.path(), &fpath)?;
                    }
                }
            }
            qf_lines.push("!static".to_owned());

            qf_lines.reverse();
            let qf_text = qf_lines.join("\n");
            let qf_data = qf_text.as_bytes();
            let mut quiltf = fs::File::create(build_dir.join("_quilt"))?;
            quiltf.write_all(&qf_data)?;
        
        }

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
        Err(e) => quilt_err(&format!("[Composition] [{}] {}", e.source, e.message))
    }

    match job.build() {
        Ok(_)  => () ,
        Err(e) => quilt_err(&format!("[Build] [{}] {}", e.source, e.message))
    }

    println!("{:#?}", job);

}
