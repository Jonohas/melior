/// Where to send the browser after a login: a path inside this application.
///
/// The value travels through the browser and the identity provider, so it is
/// untrusted. Only app-relative paths are accepted, which rules out open redirects,
/// and the login endpoints themselves are refused, which rules out redirect loops.
/// Anything else falls back to the application root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReturnPath(String);

impl ReturnPath {
    const ROOT: &'static str = "/";
    const RESERVED: [&'static str; 4] = ["/login", "/signin", "/signout", "/api"];

    #[must_use]
    pub fn root() -> Self {
        Self(Self::ROOT.to_owned())
    }

    /// Returns the path if it is acceptable, the root otherwise.
    #[must_use]
    pub fn parse(value: &str) -> Self {
        if Self::is_safe(value) { Self(value.to_owned()) } else { Self::root() }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn is_safe(value: &str) -> bool {
        let app_relative = value.starts_with('/')
            && !value.starts_with("//")
            && !value.starts_with("/.")
            && !value.contains("://")
            && !value.contains("..")
            && !value.contains('\\')
            && !value.chars().any(char::is_control);
        let reserved = Self::RESERVED.iter().any(|prefix| {
            value == *prefix
                || value
                    .strip_prefix(prefix)
                    .is_some_and(|rest| rest.starts_with('/') || rest.starts_with('?'))
        });
        app_relative && !reserved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_app_relative_paths() {
        for ok in ["/", "/rooms/general", "/rooms/general?relay=true#top", "/loginhelp"] {
            assert_eq!(ReturnPath::parse(ok).as_str(), ok);
        }
    }

    #[test]
    fn falls_back_to_root_for_anything_else() {
        for bad in [
            "",
            "rooms/general",
            "https://evil.example/",
            "//evil.example/",
            "/\\evil.example",
            "/./x",
            "/a/../b",
            "/x?next=http://evil.example",
            "/login",
            "/login?redirectTo=/",
            "/signin/x",
            "/signout",
            "/api",
            "/api/identity/me",
            "/rooms\r\nSet-Cookie: x=1",
        ] {
            assert_eq!(ReturnPath::parse(bad), ReturnPath::root(), "{bad:?}");
        }
    }
}
