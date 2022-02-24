use std::fs;
use std::path::Path;
use std::env;

fn main() {
    copy_acid64pro_library_to_build_folder();
}

fn copy_acid64pro_library_to_build_folder() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let build_folder_root = Path::new(&out_dir).parent().unwrap().parent().unwrap().parent().unwrap();

    if build_folder_root.exists() {
        let _ = fs::copy("./library/acid64pro.dll", build_folder_root.join("acid64pro.dll").to_str().unwrap());
        let _ = fs::copy("./library/hardsid_usb.dll", build_folder_root.join("hardsid_usb.dll").to_str().unwrap());
    }
}
