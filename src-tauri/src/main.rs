// release 下不弹出额外的控制台窗口（Windows），别删！
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    godbbs_launcher_lib::run()
}
