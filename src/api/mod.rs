//! 官方服务器协议（运动世界校园版）。
//!
//! 轨迹生成、签名、提交、登录与极验算法均沿用原实现，不做改动。

pub mod account;
pub mod client;
pub mod deviceverify;
pub mod facecheck;
pub mod fence;
pub mod flow;
pub mod gt4;
pub mod gt4image;
pub mod login;
pub mod model;
pub mod obs;
pub mod points;
pub mod policy;
pub mod records;
pub mod semester;
pub mod submit;
pub mod user;
