#![feature(type_ascription)]

extern crate walkdir;
extern crate pulldown_cmark;
extern crate time;
extern crate toml;

#[macro_use]
extern crate serde_derive;

use std::convert;
use std::env;
use std::process;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf, Component};
use std::ffi::OsStr;
use std::collections::{HashSet, HashMap};
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


fn copy_dir<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> Result<(), QuiltError> {
    let from_path = from.as_ref();
    let to_path   = to.as_ref();

    for entry in walkdir::WalkDir::new(from_path) {
        let entry = entry?;

        if entry.path().is_file() {
            let thread = entry.path().strip_prefix(from_path).unwrap();
            let dirs   = thread.parent().unwrap();
            let linked = to_path.join(dirs);
            fs::create_dir_all(&linked)?;

            let fpath = linked.join(entry.path().file_name().unwrap());
            fs::copy(entry.path(), &fpath)?;
        }
   }
   Ok(())
}

#[derive(Debug, Deserialize)]
struct PageToml {
    theme    : Option<String>,
    template : Option<String>,
}

impl PageToml {
    fn empty() -> Self {
        PageToml {theme: None, template: None}
    }
}

#[derive(Debug)]
struct Page {
    pub name       : String,
    pub section_id : usize ,
    pub page_toml  : PageToml,
    pub has_toml   : bool  ,
    pub has_md     : bool  ,
}

impl Page {
    fn generate<'buf>(&self, in_buf: & 'buf str) -> String {
            let parser = markdown::Parser::new(&in_buf);
            let mut out_buf = String::new();
            markdown::html::push_html(&mut out_buf, parser);
            out_buf
    }

}

#[derive(Debug)]
struct Site {
    site_dir   : PathBuf,
    static_dir : Option<PathBuf>,
    themes_dir : Option<PathBuf>,
    sections   : Vec<PathBuf>,
    pages      : HashMap<PathBuf, Page>,    
}

impl Site {
    fn init(from_path: PathBuf) -> Self {
        Site {
            site_dir  : from_path.join("site"),
            static_dir: None,
            themes_dir: None,
            sections  : vec![],
            pages     : HashMap::new(),
        }
    }
}

#[derive(Debug)]
struct Job<'args> {
    from_path : &'args str   ,
    to_path   : &'args str   ,
    site      : Site    ,
}

impl<'args> Job<'args> {
    fn init(from_path: &'args str, to_path: &'args str) -> Self {
        let site = Site::init(PathBuf::from(from_path));

        Job {
            from_path: from_path,
            to_path  : to_path  ,
            site     : site     ,
        }
    }

    fn compose(&mut self) -> Result<(), QuiltError> {
        let mut cursec_path = PathBuf::from("site");
        let mut cursec_id   = 0;

        let mut has_themes: bool = false;
        let mut has_static: bool = false;
        let mut has_site  : bool = false;
        let mut sections  : Vec<PathBuf>    = vec![cursec_path.clone()];

        let site_dir   = self.site.site_dir.clone();
        let themes_dir = PathBuf::from(self.from_path).join("themes");
        let static_dir = PathBuf::from(self.from_path).join("static");
        
        {    
            let mut pages = &mut self.site.pages;

            let from_dir   = PathBuf::from(self.from_path);

            let mut await_context = false;
            let mut mapping = false;
            
            for entry in walkdir::WalkDir::new(self.from_path) {
                let entry = entry?;

                if entry.path() == from_dir {
                    continue;
                }

                let parent = entry.path().parent().unwrap();
                
                if parent  == from_dir && entry.file_type().is_dir() {
                    await_context = false;
                    if entry.path() == static_dir {
                        mapping = false;
                        has_static = true;
                        await_context = true;
                        continue
                    }

                    if entry.path() == site_dir {
                        has_site = true;
                        mapping = true;
                        continue
                    }

                    if entry.path() == themes_dir {
                        has_themes = true;
                        mapping = false;
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
                        let mut page : &mut Page;

                        if pages.contains_key(&page_path) {
                            page = pages.get_mut(&page_path).unwrap();
                            
                            if page.has_md {
                               quilt_assert(ext == "toml",
                                            &format!("Unexpected File: {:?}", entry.path()));
                               page.has_toml = true;

                           }
                           else if page.has_toml {
                               quilt_assert(ext == "md",
                                            &format!("Unexpected File: {:?}", entry.path()));
                               page.has_md = true;
                           }
                        }
                        else {
                            let (is_md, is_toml) = (ext == "md", ext == "toml");

                            if is_md || is_toml {
                                let new_page  = Page {name: name.to_str().unwrap().to_owned(),
                                                      section_id: cursec_id,
                                                      page_toml : PageToml::empty(),
                                                      has_md    : is_md    , 
                                                      has_toml  : is_toml  ,         };
                                {
                                    pages.insert(page_path.clone(), new_page);
                                }
                                page = pages.get_mut(&page_path).unwrap();
                            }    
                            else {
                                quilt_err(&format!("{:?} is not a valid pagefile.", ext));
                            }
                        }

                        if ext == "toml" {
                            let mut toml_buf = String::new();
                            let mut toml_f   = fs::File::open(entry.path())?;
                            toml_f.read_to_string(&mut toml_buf)?;
                            println!("{}", toml_buf);
                            match toml::from_str(&toml_buf) : Result<PageToml, toml::de::Error> {
                                Ok(pt) => {page.page_toml = pt},
                                Err(err)          => {
                                                       let message = format!("Could not decode {}", entry.path().display());
                                                       let qerr = QuiltError {source: "Toml".to_owned(), message: message};
                                                       return Err(qerr);
                                                     },
                            } 
                        }

                    }
                }
            }
        }

        quilt_assert(has_site, "/site directory not found");

        let static_opt = if has_static { Some(static_dir) } else { None };
        let themes_opt = if has_themes { Some(themes_dir) } else { None };

        let site = &mut self.site;
        
        site.static_dir = static_opt;
        site.themes_dir = themes_opt;
        site.sections   = sections;
        
        Ok(())
    }

    fn build(&mut self) -> Result<(), QuiltError> {
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

        let site: &Site = &self.site;

        let mut qf_lines : Vec<String> = vec![];

        for section in &site.sections {
            let adj = section.strip_prefix("site").unwrap();
            fs::create_dir_all(build_dir.join(adj))?;

            if let Some(Component::Normal(ref s)) = adj.components().next() {
                qf_lines.push(s.to_str().unwrap().to_owned());
            }
        }

        let mut found_themes : HashMap<String, HashSet<String>> = HashMap::new();

        for (path, page) in &site.pages {
             if !page.has_md {
                 println!("Page {} ({}) does not have an associated markdown file - skipping.", page.name, path.display());
                 continue;
             }

             let ptoml = &page.page_toml;
             if let (&Some(ref theme), &Some(ref temp)) = (&ptoml.theme, &ptoml.template) {
                  if found_themes.contains_key(theme) {
                      found_themes.get_mut(theme).unwrap().insert(temp.to_owned());
                  }
                  else {
                      let mut set = HashSet::new();
                      set.insert(temp.to_owned());
                      found_themes.insert(theme.to_owned(), set);
                  }
             }
             else {
                 println!("{:?}", &page.page_toml);
             }

             let adjusted_path = path.strip_prefix("site").unwrap().to_path_buf();

             let mut md_path = site.site_dir.join(&adjusted_path);
             md_path.set_extension("md");
             
             let mut page_md = fs::File::open(md_path)?;
             
             let mut md_buf = String::new();
             page_md.read_to_string(&mut md_buf);
             let html_buf = page.generate(&md_buf);

             let mut html_path = build_dir.join(adjusted_path);
             html_path.set_extension("html");
             qf_lines.push(html_path.strip_prefix(&build_dir)
                                    .unwrap()
                                    .to_str()
                                    .unwrap()
                                    .to_owned());
             
             println!("Writing HTML: {}", html_path.display());
             let mut page_html = fs::File::create(html_path)?;
             page_html.write_all(&html_buf.as_bytes())?;
        }

        let tmp_dir    = build_dir.join(".quilt_tmp");
        let tmp_static = tmp_dir.join("static");
        if let Some(ref static_dir) = site.static_dir {
            copy_dir(static_dir, &tmp_static)?;
        }
        else {
            fs::create_dir(&tmp_static)?;
        }

        if let Some(ref theme_path) = site.themes_dir {
            let themes_data = tmp_static.join("themes");
            fs::create_dir(&themes_data);
            for (theme, temps) in &found_themes {
                let theme_dir = theme_path.join(theme);
                if theme_dir.exists() {
                    let theme_static = theme_dir.join("static");
                    if theme_static.exists() {
                        copy_dir(&theme_static, &themes_data.join(theme));
                    }
                    else {
                        fs::create_dir(&themes_data.join(theme));
                    }

                    for temp in temps {
                        let temp_dir  = theme_dir.join(temp);
                        if temp_dir.exists() {
                            let temp_data = themes_data.join(theme).join(temp);
                            copy_dir(&temp_dir, &temp_data);
                        }
                    }
                }
            }
        }

        copy_dir(&tmp_static, build_dir.join("static"))?;
        fs::remove_dir_all(&tmp_dir)?;

        qf_lines.push("!static".to_owned());

        qf_lines.reverse();
        let qf_text = qf_lines.join("\n");
        let qf_data = qf_text.as_bytes();
        let mut quiltf = fs::File::create(build_dir.join("_quilt"))?;
        quiltf.write_all(&qf_data)?;

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
    
    let mut job = Job::init(from, to);
    
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
