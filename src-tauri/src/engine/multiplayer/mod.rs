//! 联机：接入陶瓦联机（Terracotta，AGPL-3.0 附例外条款）。
//!
//! 我们不改、不链接陶瓦，只下载它未经修改的官方发行包、作为独立进程运行，通过它的本机 HTTP 接口控制；
//! 按它的例外条款，界面上要显眼地标注陶瓦的版权信息（联机页底部和「关于」）。设计见 docs/MULTIPLAYER.md。

pub mod api;
pub mod process;
pub mod release;
pub mod room;
