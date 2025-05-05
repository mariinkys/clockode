pub fn main() {
    println!("cargo::rerun-if-changed=fonts/iced-twofa.toml");
    iced_fontello::build("fonts/iced-twofa.toml").expect("Build iced-twofa font");
}
