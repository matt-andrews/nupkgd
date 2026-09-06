use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema, Default)]
pub struct NugetVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub revision: u32,
    pub prerelease: Option<String>,
    pub metadata: Option<String>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Identifier {
    Numeric(u64),
    Alpha(String),
}

impl Identifier {
    fn parse(s: &str) -> Self {
        if s.bytes().all(|b| b.is_ascii_digit())
            && let Ok(n) = s.parse::<u64>() {
                return Identifier::Numeric(n);
            }
        Identifier::Alpha(s.to_ascii_lowercase())
    }
}

fn label(name: &str, value: Option<&str>) -> anyhow::Result<Option<String>> {
    let Some(value) = value else { return Ok(None) };
    let valid = !value.is_empty()
        && value.split('.').all(|id| {
        !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    });

    if !valid {
        anyhow::bail!("invalid {name} label {value:?}");
    }

    Ok(Some(value.to_lowercase().to_owned()))
}

impl FromStr for NugetVersion {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (s, metadata) = s.split_once('+').map_or((s, None), |(v, m)| (v, Some(m)));
        let (s, prerelease) = s.split_once('-').map_or((s, None), |(v, p)| (v, Some(p)));

        let mut parts = s.split('.').map(|p|{
            p.parse::<u32>()
                .map_err(|e| anyhow::anyhow!("invalid version component {p:?}: {e}"))
        });

        let major = parts.next().unwrap()?;
        let minor = parts.next().transpose()?.unwrap_or(0);
        let patch = parts.next().transpose()?.unwrap_or(0);
        let revision = parts.next().transpose()?.unwrap_or(0);
        if parts.next().is_some() {
            anyhow::bail!("version has more than four numeric components");
        }

        Ok(NugetVersion{
            major,
            minor,
            patch,
            revision,
            prerelease: label("prerelease", prerelease)?,
            metadata: label("metadata", metadata)?,
        })
    }
}

impl NugetVersion{
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        s.parse()
    }

    fn precedence_key(&self) -> (u32, u32, u32, u32, Option<Vec<Identifier>>) {
        let labels = self
            .prerelease
            .as_deref()
            .map(|p| p.split('.').map(Identifier::parse).collect());
        (self.major, self.minor, self.patch, self.revision, labels)
    }
}

impl fmt::Display for NugetVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;

        if self.revision > 0 {
            write!(f, ".{}", self.revision)?;
        }

        if let Some(pre) = &self.prerelease {
            write!(f, "-{}", pre)?;
        }

        Ok(())
    }
}

impl Ord for NugetVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch, self.revision)
            .cmp(&(other.major, other.minor, other.patch, other.revision))
            .then_with(|| match (&self.prerelease, &other.prerelease) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater, // release beats prerelease
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) => a
                    .split('.')
                    .map(Identifier::parse)
                    .cmp(b.split('.').map(Identifier::parse)),
            })
    }
}

impl PartialOrd for NugetVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for NugetVersion {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for NugetVersion {}

// Only needed if you'll use versions as HashMap/HashSet keys.
// Hashes exactly what cmp compares, so equal versions hash equally.
impl Hash for NugetVersion {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.precedence_key().hash(state);
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;
    use crate::models::nuget_version::NugetVersion;

    fn assert_version(
        version: NugetVersion,
        major: u32,
        minor: u32,
        patch: u32,
        revision: u32,
        prerelease: Option<String>,
        metadata: Option<String>
    ) {
        assert_eq!(version.major, major);
        assert_eq!(version.minor, minor);
        assert_eq!(version.patch, patch);
        assert_eq!(version.revision, revision);
        assert_eq!(version.prerelease, prerelease);
        assert_eq!(version.metadata, metadata);
    }

    #[test]
    fn test_parse_no_minor() {
        let parse = "1";
        let version = NugetVersion::parse(parse).unwrap();
        assert_version(version, 1, 0, 0, 0, None, None);
    }

    #[test]
    fn test_parse_no_patch() {
        let parse = "1.2";
        let version = NugetVersion::parse(parse).unwrap();
        assert_version(version, 1, 2, 0, 0, None, None);
    }

    #[test]
    fn test_parse_no_revision() {
        let parse = "1.2.3";
        let version = NugetVersion::parse(parse).unwrap();
        assert_version(version, 1, 2, 3, 0, None, None);
    }

    #[test]
    fn test_parse_with_revision() {
        let parse = "1.2.3.4";
        let version = NugetVersion::parse(parse).unwrap();
        assert_version(version, 1, 2, 3, 4, None, None);
    }

    #[test]
    fn test_parse_with_leading_zeroes() {
        let parse = "01.02.03.04";
        let version = NugetVersion::parse(parse).unwrap();
        assert_version(version, 1, 2, 3, 4, None, None);
    }

    #[test]
    fn test_parse_with_metadata() {
        let parse = "1.2.3+meta123";
        let version = NugetVersion::parse(parse).unwrap();
        assert_version(version, 1, 2, 3, 0, None, Some("meta123".to_string()));
    }

    #[test]
    fn test_parse_with_prerelease() {
        let parse = "1.2.3-prerelease";
        let version = NugetVersion::parse(parse).unwrap();
        assert_version(version, 1, 2, 3, 0, Some("prerelease".to_string()), None);
    }

    #[test]
    fn test_parse_with_prerelease_and_metadata() {
        let parse = "1.2.3-prerelease+meta123";
        let version = NugetVersion::parse(parse).unwrap();
        assert_version(version, 1, 2, 3, 0, Some("prerelease".to_string()), Some("meta123".to_string()));
    }

    #[test]
    fn test_parse_too_many_prereleases() {
        let parse = "1.2.3-prerelease-meta123";
        let version = NugetVersion::parse(parse).unwrap();
        assert_version(version, 1, 2, 3, 0, Some("prerelease-meta123".to_string()), None);
    }

    #[test]
    fn test_parse_too_many_metadata() {
        let parse = "1.2.3+prerelease+meta123";
        let version = NugetVersion::parse(parse);
        assert!(version.is_err());
    }

    #[test]
    fn test_parse_too_many_sections() {
        let parse = "1.2.3.4.5";
        let version = NugetVersion::parse(parse);
        assert!(version.is_err());
    }

    #[test]
    fn test_parse_oops_all_letters() {
        let parse = "abc";
        let version = NugetVersion::parse(parse);
        assert!(version.is_err());
    }

    #[test]
    fn test_parse_empty_minor() {
        let parse = "1..3";
        let version = NugetVersion::parse(parse);
        assert!(version.is_err());
    }

    #[test]
    fn test_parse_empty_prerelease() {
        let parse = "1.2.3-";
        let version = NugetVersion::parse(parse);
        assert!(version.is_err());
    }

    #[test]
    fn test_parse_empty_metadata() {
        let parse = "1.2.3+";
        let version = NugetVersion::parse(parse);
        assert!(version.is_err());
    }

    #[test]
    fn test_parse_empty_fails() {
        let parse = "";
        let version = NugetVersion::parse(parse);
        assert!(version.is_err());
    }

    #[test]
    fn test_parse_overflow_fails() {
        let parse = "1.2.3.99999999999";
        let version = NugetVersion::parse(parse);
        assert!(version.is_err());
    }

    #[test]
    fn test_to_string_major(){
        let parse = "1";
        let result = "1.0.0";
        let version = NugetVersion::parse(parse).unwrap();
        assert_eq!(version.to_string(), result);
    }

    #[test]
    fn test_to_string_minor(){
        let parse = "1.2";
        let result = "1.2.0";
        let version = NugetVersion::parse(parse).unwrap();
        assert_eq!(version.to_string(), result);
    }

    #[test]
    fn test_to_string_patch(){
        let parse = "1.2.3";
        let result = "1.2.3";
        let version = NugetVersion::parse(parse).unwrap();
        assert_eq!(version.to_string(), result);
    }

    #[test]
    fn test_to_string_revision(){
        let parse = "1.2.3.4";
        let result = "1.2.3.4";
        let version = NugetVersion::parse(parse).unwrap();
        assert_eq!(version.to_string(), result);
    }

    #[test]
    fn test_to_string_prerelease(){
        let parse = "1.2.3.4-prerelease";
        let result = "1.2.3.4-prerelease";
        let version = NugetVersion::parse(parse).unwrap();
        assert_eq!(version.to_string(), result);
    }

    #[test]
    fn test_to_string_metadata(){
        let parse = "1.2.3.4+meta123";
        let result = "1.2.3.4";
        let version = NugetVersion::parse(parse).unwrap();
        assert_eq!(version.to_string(), result);
    }

    #[test]
    fn test_to_string_prerelease_and_metadata(){
        let parse = "1.2.3.4-prerelease+meta123";
        let result = "1.2.3.4-prerelease";
        let version = NugetVersion::parse(parse).unwrap();
        assert_eq!(version.to_string(), result);
    }

    #[test]
    fn test_to_string_metadata_no_revision(){
        let parse = "1.2.3+meta123";
        let result = "1.2.3";
        let version = NugetVersion::parse(parse).unwrap();
        assert_eq!(version.to_string(), result);
    }

    #[test]
    fn test_to_string_metadata_no_patch(){
        let parse = "1.2+meta123";
        let result = "1.2.0";
        let version = NugetVersion::parse(parse).unwrap();
        assert_eq!(version.to_string(), result);
    }

    #[test]
    fn test_to_string_metadata_no_minor(){
        let parse = "1+meta123";
        let result = "1.0.0";
        let version = NugetVersion::parse(parse).unwrap();
        assert_eq!(version.to_string(), result);
    }

    #[test]
    fn test_to_string_prerelease_and_metadata_leading_zeroes(){
        let parse = "01.02.03.04-prerelease+meta123";
        let result = "1.2.3.4-prerelease";
        let version = NugetVersion::parse(parse).unwrap();
        assert_eq!(version.to_string(), result);
    }

    #[test]
    fn test_to_string_prerelease_and_metadata_with_version_parts(){
        let parse = "1.2.3.4-beta.1+build.5";
        let result = "1.2.3.4-beta.1";
        let version = NugetVersion::parse(parse).unwrap();
        assert_eq!(version.to_string(), result);
    }

    #[test]
    fn test_to_string_prerelease_with_version_parts(){
        let parse = "9.0.0-Preview.1.24080.9";
        let result = "9.0.0-preview.1.24080.9";
        let version = NugetVersion::parse(parse).unwrap();
        assert_eq!(version.to_string(), result);
    }

    #[test]
    fn test_to_string_long_patch(){
        let version = "1.0.20240903".parse::<NugetVersion>().unwrap();
        let result = "1.0.20240903";
        assert_eq!(version.to_string(), result);
    }

    #[test]
    fn test_to_string_drops_zero_revision() {
        assert_eq!(NugetVersion::parse("1.2.3.0").unwrap().to_string(), "1.2.3");
    }

    #[test]
    fn test_equivalent_forms_are_equal() {
        let a = NugetVersion::parse("1.0").unwrap();
        let b = NugetVersion::parse("1.0.0.0").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_parse_component_overflow() {
        assert!(NugetVersion::parse("1.2.3.99999999999").is_err());
    }

    #[test]
    fn test_parse_invalid_label_chars() {
        assert!(NugetVersion::parse("1.0.0-beta_1").is_err());
        assert!(NugetVersion::parse("1.0.0-beta..1").is_err());
        assert!(NugetVersion::parse("1.0.0+build-1").is_ok());
    }

    #[test]
    fn test_round_trip() {
        for s in ["1.0.0", "1.2.3.4", "1.0.0-beta.1", "9.0.0-preview.1.24080.9"] {
            let v = NugetVersion::parse(s).unwrap();
            assert_eq!(NugetVersion::parse(&v.to_string()).unwrap(), v);
        }
    }

    #[test]
    fn test_ordering_semver_precedence_chain() {
        let ordered = [
            "1.0.0-alpha", "1.0.0-alpha.1", "1.0.0-alpha.beta", "1.0.0-beta",
            "1.0.0-beta.2", "1.0.0-beta.11", "1.0.0-rc.1", "1.0.0", "1.0.0.1", "1.0.1",
        ];
        let versions: Vec<_> = ordered.iter().map(|s| NugetVersion::parse(s).unwrap()).collect();
        for pair in versions.windows(2) {
            assert!(pair[0] < pair[1], "{} should sort before {}", pair[0], pair[1]);
        }
    }

    #[test]
    fn test_ordering_ignores_metadata_and_case() {
        assert_eq!(NugetVersion::parse("1.0.0+a").unwrap(), NugetVersion::parse("1.0.0+b").unwrap());
        assert_eq!(NugetVersion::parse("1.0.0-BETA").unwrap(), NugetVersion::parse("1.0.0-beta").unwrap());
    }

    #[test]
    fn test_ordering_consistent_with_equality() {
        let inputs = ["1.0.0", "1.0.0+x", "1.0.0-beta", "1.0.0-Beta", "1.0.0-beta.1", "1.0.0.0"];
        let versions: Vec<_> = inputs.iter().map(|s| NugetVersion::parse(s).unwrap()).collect();
        for a in &versions {
            for b in &versions {
                assert_eq!(a == b, a.cmp(b) == Ordering::Equal, "{a} vs {b}");
            }
        }
    }
}