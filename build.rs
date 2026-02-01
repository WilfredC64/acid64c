use std::fs;
use std::path::Path;
use std::env;

fn main() {
    #[cfg(windows)] {
        let mut res = winres::WindowsResource::new();
        res.set_icon("resources/acid64.ico");
        res.compile().unwrap();
    }

    copy_acid64pro_library_to_build_folder();
    println!("cargo:rerun-if-changed=library/");
}

fn copy_acid64pro_library_to_build_folder() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let build_folder_root = Path::new(&out_dir).parent().unwrap().parent().unwrap().parent().unwrap();

    if build_folder_root.exists() {
        let _ = fs::copy("./library/acid64pro.dll", build_folder_root.join("acid64pro.dll").to_str().unwrap());
        let _ = fs::copy("./library/hardsid_usb.dll", build_folder_root.join("hardsid_usb.dll").to_str().unwrap());
        let _ = fs::copy("./library/libacid64pro.so", build_folder_root.join("libacid64pro.so").to_str().unwrap());
        let _ = fs::copy("./library/libacid64pro.dylib", build_folder_root.join("libacid64pro.dylib").to_str().unwrap());
    }
}
