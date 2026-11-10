//! OAuth against a fake issuer: discovery, dynamic client registration, PKCE, the loopback
//! callback and the token exchange, all real HTTP over a real socket on this machine.
//!
//! No live authorization server and no browser are involved: the test plays the browser, which
//! is the only part of the flow a machine cannot do for itself.

mod flow;
mod issuer;
mod pure;
