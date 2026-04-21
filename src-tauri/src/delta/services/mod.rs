pub mod game;
pub mod qq_auth;
pub mod qq_safe;
pub mod wegame_auth;
pub mod wechat_auth;

pub use game::{GameAuth, GameService};
pub use qq_auth::{QqAccessToken, QqAuthService, QqLoginQr, QqStatusRequest, UpdateTokenOnlyRequest};
pub use qq_safe::{QqSafeAccess, QqSafeService};
pub use wechat_auth::{WechatAccessToken, WechatAuthService, WechatQr};
pub use wegame_auth::{WegameAuthService, WegameQqLoginQr, WegameQqStatusRequest, WegameTicket};
