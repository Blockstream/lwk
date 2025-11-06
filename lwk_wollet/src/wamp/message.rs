/// Copied from https://github.com/sideswap-io/sideswap_rust/ with minor modifications
/// Copied from https://github.com/elast0ny/wamp_async with minor modifications
/// [MIT license](http://opensource.org/licenses/MIT)
///
use std::fmt;

use serde::de::{Deserializer, Error, SeqAccess, Visitor};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};

use crate::wamp::common::*;

// Message IDs
pub const HELLO_ID: WampInteger = 1;
pub const WELCOME_ID: WampInteger = 2;
pub const ABORT_ID: WampInteger = 3;
pub const GOODBYE_ID: WampInteger = 6;
pub const ERROR_ID: WampInteger = 8;
pub const SUBSCRIBE_ID: WampInteger = 32;
pub const SUBSCRIBED_ID: WampInteger = 33;
pub const UNSUBSCRIBE_ID: WampInteger = 34;
pub const UNSUBSCRIBED_ID: WampInteger = 35;
pub const EVENT_ID: WampInteger = 36;
pub const CALL_ID: WampInteger = 48;
pub const RESULT_ID: WampInteger = 50;

/// WAMP message
#[derive(Debug)]
pub enum Msg {
    /// Sent by a Client to initiate opening of a WAMP session to a Router attaching to a Realm.
    Hello { realm: WampUri, details: WampDict },
    /// Sent by a Router to accept a Client. The WAMP session is now open.
    Welcome { session: WampId, details: WampDict },
    /// Sent by a Peer to abort the opening of a WAMP session. No response is expected.
    Abort { details: WampDict, reason: WampUri },
    /// Sent by a Peer to close a previously opened WAMP session. Must be echo'ed by the receiving Peer.
    Goodbye { details: WampDict, reason: WampUri },
    /// Error reply sent by a Peer as an error response to different kinds of requests.
    Error {
        typ: WampInteger,
        request: WampId,
        details: WampDict,
        error: WampUri,
        arguments: Option<WampArgs>,
        arguments_kw: Option<WampKwArgs>,
    },
    /// Subscribe request sent by a Subscriber to a Broker to subscribe to a topic.
    Subscribe {
        request: WampId,
        options: WampDict,
        topic: WampUri,
    },
    /// Acknowledge sent by a Broker to a Subscriber to acknowledge a subscription.
    Subscribed {
        request: WampId,
        subscription: WampId,
    },
    /// Unsubscribe request sent by a Subscriber to a Broker to unsubscribe a subscription.
    Unsubscribe {
        request: WampId,
        subscription: WampId,
    },
    /// Acknowledge sent by a Broker to a Subscriber to acknowledge unsubscription.
    Unsubscribed { request: WampId },
    /// Event dispatched by Broker to Subscribers for subscriptions the event was matching.
    Event {
        subscription: WampId,
        publication: WampId,
        details: WampDict,
        arguments: Option<WampArgs>,
        arguments_kw: Option<WampKwArgs>,
    },
    /// Call as originally issued by the Caller to the Dealer.
    Call {
        request: WampId,
        options: WampDict,
        procedure: WampUri,
        arguments: Option<WampArgs>,
        arguments_kw: Option<WampKwArgs>,
    },
    /// Result of a call as returned by Dealer to Caller.
    Result {
        request: WampId,
        details: WampDict,
        arguments: Option<WampArgs>,
        arguments_kw: Option<WampKwArgs>,
    },
}

impl Msg {
    pub fn request_id(&self) -> Option<WampId> {
        Some(*match self {
            Msg::Error { ref request, .. } => request,
            Msg::Subscribe { ref request, .. } => request,
            Msg::Subscribed { ref request, .. } => request,
            Msg::Unsubscribe { ref request, .. } => request,
            Msg::Unsubscribed { ref request } => request,
            Msg::Call { ref request, .. } => request,
            Msg::Result { ref request, .. } => request,
            Msg::Hello { .. }
            | Msg::Welcome { .. }
            | Msg::Abort { .. }
            | Msg::Goodbye { .. }
            | Msg::Event { .. } => return None,
        })
    }
}

/// Serialization from the struct to the WAMP tuple
impl Serialize for Msg {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Converts the enum struct to a tuple representation
        match self {
            Msg::Hello {
                ref realm,
                ref details,
            } => (HELLO_ID, realm, details).serialize(serializer),
            Msg::Welcome {
                ref session,
                ref details,
            } => (WELCOME_ID, session, details).serialize(serializer),
            Msg::Abort {
                ref details,
                ref reason,
            } => (ABORT_ID, details, reason).serialize(serializer),
            Msg::Goodbye {
                ref details,
                ref reason,
            } => (GOODBYE_ID, details, reason).serialize(serializer),
            Msg::Error {
                ref typ,
                ref request,
                ref details,
                ref error,
                ref arguments,
                ref arguments_kw,
            } => {
                if let Some(arguments_kw) = arguments_kw {
                    (
                        ERROR_ID,
                        typ,
                        request,
                        details,
                        error,
                        arguments.as_ref().unwrap_or(&WampArgs::new()),
                        arguments_kw,
                    )
                        .serialize(serializer)
                } else if let Some(arguments) = arguments {
                    (ERROR_ID, typ, request, details, error, arguments).serialize(serializer)
                } else {
                    (ERROR_ID, typ, request, details, error).serialize(serializer)
                }
            }
            Msg::Subscribe {
                ref request,
                ref options,
                ref topic,
            } => (SUBSCRIBE_ID, request, options, topic).serialize(serializer),
            Msg::Subscribed {
                ref request,
                ref subscription,
            } => (SUBSCRIBED_ID, request, subscription).serialize(serializer),
            Msg::Unsubscribe {
                ref request,
                ref subscription,
            } => (UNSUBSCRIBE_ID, request, subscription).serialize(serializer),
            Msg::Unsubscribed { ref request } => (UNSUBSCRIBED_ID, request).serialize(serializer),
            Msg::Event {
                ref subscription,
                ref publication,
                ref details,
                ref arguments,
                ref arguments_kw,
            } => {
                if let Some(arguments_kw) = arguments_kw {
                    (
                        EVENT_ID,
                        subscription,
                        publication,
                        details,
                        arguments.as_ref().unwrap_or(&WampArgs::new()),
                        arguments_kw,
                    )
                        .serialize(serializer)
                } else if let Some(arguments) = arguments {
                    (EVENT_ID, subscription, publication, details, arguments).serialize(serializer)
                } else {
                    (EVENT_ID, subscription, publication, details).serialize(serializer)
                }
            }
            Msg::Call {
                ref request,
                ref options,
                ref procedure,
                ref arguments,
                ref arguments_kw,
            } => {
                if let Some(arguments_kw) = arguments_kw {
                    (
                        CALL_ID,
                        request,
                        options,
                        procedure,
                        arguments.as_ref().unwrap_or(&WampArgs::new()),
                        arguments_kw,
                    )
                        .serialize(serializer)
                } else if let Some(arguments) = arguments {
                    (CALL_ID, request, options, procedure, arguments).serialize(serializer)
                } else {
                    (CALL_ID, request, options, procedure).serialize(serializer)
                }
            }
            Msg::Result {
                ref request,
                ref details,
                ref arguments,
                ref arguments_kw,
            } => {
                if let Some(arguments_kw) = arguments_kw {
                    (
                        RESULT_ID,
                        request,
                        details,
                        arguments.as_ref().unwrap_or(&WampArgs::new()),
                        arguments_kw,
                    )
                        .serialize(serializer)
                } else if let Some(arguments) = arguments {
                    (RESULT_ID, request, details, arguments).serialize(serializer)
                } else {
                    (RESULT_ID, request, details).serialize(serializer)
                }
            }
        }
    }
}

/// Deserialization from the WAMP tuple to the struct
impl<'de> Deserialize<'de> for Msg {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct MsgVisitor;
        impl MsgVisitor {
            fn de_hello<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Hello {
                    realm: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("realm"))?,
                    details: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("details"))?,
                })
            }
            fn de_welcome<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Welcome {
                    session: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("session"))?,
                    details: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("details"))?,
                })
            }
            fn de_abort<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Abort {
                    details: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("details"))?,
                    reason: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("reason"))?,
                })
            }
            fn de_goodbye<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Goodbye {
                    details: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("details"))?,
                    reason: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("reason"))?,
                })
            }
            fn de_error<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Error {
                    typ: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("type"))?,
                    request: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("request"))?,
                    details: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("details"))?,
                    error: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("error"))?,
                    arguments: v.next_element()?.unwrap_or(None),
                    arguments_kw: v.next_element()?.unwrap_or(None),
                })
            }
            fn de_subscribe<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Subscribe {
                    request: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("request"))?,
                    options: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("options"))?,
                    topic: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("topic"))?,
                })
            }
            fn de_subscribed<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Subscribed {
                    request: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("request"))?,
                    subscription: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("subscription"))?,
                })
            }
            fn de_unsubscribe<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Unsubscribe {
                    request: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("request"))?,
                    subscription: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("subscription"))?,
                })
            }
            fn de_unsubscribed<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Unsubscribed {
                    request: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("request"))?,
                })
            }
            fn de_event<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Event {
                    subscription: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("subscription"))?,
                    publication: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("publication"))?,
                    details: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("details"))?,
                    arguments: v.next_element()?.unwrap_or(None),
                    arguments_kw: v.next_element()?.unwrap_or(None),
                })
            }
            fn de_call<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Call {
                    request: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("request"))?,
                    options: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("options"))?,
                    procedure: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("procedure"))?,
                    arguments: v.next_element()?.unwrap_or(None),
                    arguments_kw: v.next_element()?.unwrap_or(None),
                })
            }
            fn de_result<'de, V: SeqAccess<'de>>(&self, mut v: V) -> Result<Msg, V::Error> {
                Ok(Msg::Result {
                    request: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("request"))?,
                    details: v
                        .next_element()?
                        .ok_or_else(|| Error::missing_field("details"))?,
                    arguments: v.next_element()?.unwrap_or(None),
                    arguments_kw: v.next_element()?.unwrap_or(None),
                })
            }
        }
        impl<'de> Visitor<'de> for MsgVisitor {
            type Value = Msg;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("WAMP message")
            }

            fn visit_seq<V>(self, mut v: V) -> Result<Msg, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let msg_id: WampInteger = v
                    .next_element()?
                    .ok_or_else(|| Error::invalid_length(0, &self))?;

                match msg_id {
                    HELLO_ID => self.de_hello(v),
                    WELCOME_ID => self.de_welcome(v),
                    ABORT_ID => self.de_abort(v),
                    GOODBYE_ID => self.de_goodbye(v),
                    ERROR_ID => self.de_error(v),
                    SUBSCRIBE_ID => self.de_subscribe(v),
                    SUBSCRIBED_ID => self.de_subscribed(v),
                    UNSUBSCRIBE_ID => self.de_unsubscribe(v),
                    UNSUBSCRIBED_ID => self.de_unsubscribed(v),
                    EVENT_ID => self.de_event(v),
                    CALL_ID => self.de_call(v),
                    RESULT_ID => self.de_result(v),
                    id => Err(Error::custom(format!("Unknown message id : {}", id))),
                }
            }
        }

        deserializer.deserialize_seq(MsgVisitor)
    }
}
