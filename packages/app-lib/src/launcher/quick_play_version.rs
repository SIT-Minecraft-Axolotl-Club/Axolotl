use daedalus::minecraft::Version;
use serde::{Deserialize, Serialize};

// If modified, also update QuickPlayServerVersion.java
//
// Variants are declared oldest-first so the derived `Ord` ranks support
// chronologically: `Unsupported < Injected < BuiltinLegacy < Builtin`. Launch
// code compares against these bounds (`<=`, `>=`), which is what decides
// between injected server properties, the legacy `--server`/`--port` pair, and
// the built-in `--quickPlayMultiplayer` argument.
#[derive(
    Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QuickPlayServerVersion {
    Unsupported,
    Injected,
    BuiltinLegacy,
    Builtin,
}

impl QuickPlayServerVersion {
    pub fn min_version(&self) -> Option<&'static str> {
        match self {
            Self::Unsupported => None,
            Self::Injected => Some("a1.0.5_01"),
            Self::BuiltinLegacy => Some("13w17a"),
            Self::Builtin => Some("23w14a"),
        }
    }
}

// If modified, also update QuickPlaySingleplayerVersion.java
#[derive(
    Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QuickPlaySingleplayerVersion {
    Unsupported,
    Builtin,
}

impl QuickPlaySingleplayerVersion {
    pub fn min_version(&self) -> Option<&'static str> {
        match self {
            Self::Unsupported => None,
            Self::Builtin => Some("23w14a"),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct QuickPlayVersion {
    pub server: QuickPlayServerVersion,
    pub singleplayer: QuickPlaySingleplayerVersion,
}

impl QuickPlayVersion {
    /// Conservative answer for a version whose manifest entry is unavailable:
    /// every Java 17+ release is far past `23w14a`, so quick play is built in.
    pub const ASSUMED_MODERN: Self = Self {
        server: QuickPlayServerVersion::Builtin,
        singleplayer: QuickPlaySingleplayerVersion::Builtin,
    };

    /// Answer for versions that predate every quick play transport.
    pub const UNSUPPORTED: Self = Self {
        server: QuickPlayServerVersion::Unsupported,
        singleplayer: QuickPlaySingleplayerVersion::Unsupported,
    };

    /// Classifies the quick play support of `versions[version_index]` by
    /// scanning *forward* to the earliest version that marks a support
    /// boundary, and by checking whether any boundary is at or after it at all.
    ///
    /// The manifest is ordered newest-first, so every boundary version sits at
    /// or after `version_index` for versions older than it. Scanning backwards
    /// (the previous implementation) silently skipped every boundary for any
    /// version newer than `23w14a` and therefore reported modern releases as
    /// `Injected`, disabling built-in quick play on exactly the versions that
    /// need it.
    pub fn find_version(version_index: usize, versions: &[Version]) -> Self {
        let server = QuickPlayServerVersion::classify(version_index, versions);
        let singleplayer =
            QuickPlaySingleplayerVersion::classify(version_index, versions);

        Self {
            server,
            singleplayer,
        }
    }
}

impl QuickPlayServerVersion {
    /// Walks the newest-first manifest to find the transport this version uses.
    fn classify(version_index: usize, versions: &[Version]) -> Self {
        let boundaries = versions
            .iter()
            .enumerate()
            .filter_map(|(index, version)| {
                Self::boundary_for(&version.id).map(|boundary| (index, boundary))
            })
            .collect::<Vec<_>>();

        // Sitting below the oldest boundary means the version predates every
        // quick play transport. Without this check the forward scan below finds
        // nothing and would claim built-in support for those versions too. An
        // out-of-range index means the version is not in this manifest at all,
        // which is an unknown (typically newer) version and keeps the
        // built-in default.
        if version_index < versions.len()
            && boundaries
                .last()
                .is_some_and(|(index, _)| *index < version_index)
        {
            return Self::Unsupported;
        }

        boundaries
            .iter()
            .find(|(index, _)| *index >= version_index)
            .map_or(Self::Builtin, |(_, boundary)| *boundary)
    }

    fn boundary_for(version: &str) -> Option<Self> {
        [
            Self::Builtin,
            Self::BuiltinLegacy,
            Self::Injected,
        ]
        .into_iter()
        .find(|candidate| candidate.min_version() == Some(version))
    }
}

impl QuickPlaySingleplayerVersion {
    fn classify(version_index: usize, versions: &[Version]) -> Self {
        let boundaries = versions
            .iter()
            .enumerate()
            .filter_map(|(index, version)| {
                Self::boundary_for(&version.id).map(|boundary| (index, boundary))
            })
            .collect::<Vec<_>>();

        if version_index < versions.len()
            && boundaries
                .last()
                .is_some_and(|(index, _)| *index < version_index)
        {
            return Self::Unsupported;
        }

        boundaries
            .iter()
            .find(|(index, _)| *index >= version_index)
            .map_or(Self::Builtin, |(_, boundary)| *boundary)
    }

    fn boundary_for(version: &str) -> Option<Self> {
        [Self::Builtin]
            .into_iter()
            .find(|candidate| candidate.min_version() == Some(version))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(id: &str) -> Version {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "type": "release",
            "url": "https://example.invalid/version.json",
            "time": "2024-01-01T00:00:00+00:00",
            "releaseTime": "2024-01-01T00:00:00+00:00",
            "sha1": "0000000000000000000000000000000000000000",
            "complianceLevel": 1,
        }))
        .unwrap()
    }

    /// Newest-first manifest, mirroring Mojang's ordering around the
    /// boundaries that matter.
    fn manifest() -> Vec<Version> {
        [
            "26.3",
            "1.21.10",
            "1.21.9",
            "1.21",
            "1.20",
            "23w14a",
            "1.19.4",
            "13w17a",
            "1.5.2",
            "a1.0.5_01",
            "a1.0.4",
        ]
        .into_iter()
        .map(version)
        .collect()
    }

    #[test]
    fn releases_newer_than_the_builtin_boundary_use_builtin_quick_play() {
        for id in ["26.3", "1.21.10", "1.21", "1.20"] {
            let versions = manifest();
            let index =
                versions.iter().position(|it| it.id == id).unwrap();
            let quick_play = QuickPlayVersion::find_version(index, &versions);
            assert_eq!(
                quick_play.server,
                QuickPlayServerVersion::Builtin,
                "{id} must use the built-in quick play argument"
            );
            assert_eq!(
                quick_play.singleplayer,
                QuickPlaySingleplayerVersion::Builtin,
                "{id} must use built-in quick play for singleplayer"
            );
        }
    }

    #[test]
    fn versions_before_the_builtin_boundary_keep_older_transports() {
        let versions = manifest();

        let index = versions.iter().position(|it| it.id == "1.19.4").unwrap();
        assert_eq!(
            QuickPlayVersion::find_version(index, &versions).server,
            QuickPlayServerVersion::BuiltinLegacy
        );

        let index = versions.iter().position(|it| it.id == "1.5.2").unwrap();
        assert_eq!(
            QuickPlayVersion::find_version(index, &versions).server,
            QuickPlayServerVersion::Injected
        );

        let index = versions.iter().position(|it| it.id == "a1.0.4").unwrap();
        assert_eq!(
            QuickPlayVersion::find_version(index, &versions).server,
            QuickPlayServerVersion::Unsupported
        );
    }

    #[test]
    fn unknown_versions_assume_builtin_support() {
        let versions = manifest();
        let quick_play = QuickPlayVersion::find_version(versions.len() + 5, &versions);
        assert_eq!(quick_play, QuickPlayVersion::ASSUMED_MODERN);
    }
}
