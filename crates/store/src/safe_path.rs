// SPDX-License-Identifier: MIT
//! Portable archive names. This is validation, not filesystem authorization.

use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;

use crate::ArchiveError;

/// A validated, NFC-normalized relative name with `/` separators.
///
/// Deliberately conservative budgets: 1024 UTF-8 bytes total, 240 bytes per
/// component and 64 components. No host-dependent `Path::components` parsing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SafeRelativePath {
    normalized: String,
    original: String,
}

impl SafeRelativePath {
    pub fn parse(name: &str) -> Result<Self, ArchiveError> {
        validate(name)?;
        let normalized: String = name.nfc().collect();
        validate(&normalized)?;
        Ok(Self {
            normalized,
            original: name.to_owned(),
        })
    }

    pub fn as_str(&self) -> &str {
        &self.normalized
    }
}

fn validate(name: &str) -> Result<(), ArchiveError> {
    if name.is_empty() || name.len() > 1024 || name.split('/').count() > 64 {
        return Err(ArchiveError::InvalidPath(
            "path budget: 1..1024 bytes, at most 64 components",
        ));
    }
    for part in name.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.len() > 240 {
            return Err(ArchiveError::InvalidPath(
                "component budget: 1..240 bytes; no dot or empty components",
            ));
        }
        if part.ends_with([' ', '.'])
            || part
                .chars()
                .any(|c| c.is_control() || "\\:<>\"|?*".contains(c))
        {
            return Err(ArchiveError::InvalidPath(
                "forbidden portable filename character or suffix",
            ));
        }
        // Compatibility normalization makes device-like Unicode spellings a
        // conservative rejection too (including COM superscript digits).
        let compatible: String = part.nfkc().flat_map(char::to_uppercase).collect();
        let stem = compatible
            .split('.')
            .next()
            .unwrap_or("")
            .trim_end_matches(' ');
        if matches!(stem, "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$")
            || (stem.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        {
            return Err(ArchiveError::InvalidPath("reserved Windows device name"));
        }
    }
    Ok(())
}

#[derive(Default)]
pub(crate) struct Namespace {
    nodes: HashMap<String, Node>,
    aliases: HashMap<(String, String), String>,
    string_bytes: usize,
}

struct Node {
    directory: bool,
    explicit: bool,
}

impl Namespace {
    pub(crate) fn insert(
        &mut self,
        path: &SafeRelativePath,
        directory: bool,
    ) -> Result<(), ArchiveError> {
        let components: Vec<_> = path.normalized.split('/').collect();
        let originals: Vec<_> = path.original.split('/').collect();
        let mut parent = String::new();
        for (i, component) in components.iter().enumerate() {
            let last = i + 1 == components.len();
            let is_dir = !last || directory;
            let compatible: String = component.nfkc().collect();
            // Full case fold catches multi-codepoint mappings; full uppercase
            // additionally rejects NTFS-style aliases such as dotless i / I.
            // These keys are a conservative policy, not an emulation of every
            // filesystem's Unicode tables.
            let keys = [
                caseless::default_case_fold_str(&compatible)
                    .nfc()
                    .collect::<String>(),
                compatible
                    .chars()
                    .flat_map(char::to_uppercase)
                    .nfc()
                    .collect(),
            ];
            for key in keys {
                let alias = (parent.clone(), key);
                if self
                    .aliases
                    .get(&alias)
                    .is_some_and(|old| old != originals[i])
                {
                    return Err(ArchiveError::Collision);
                }
                if !self.aliases.contains_key(&alias) {
                    self.string_bytes += alias.0.len() + alias.1.len() + originals[i].len();
                    self.check_budget()?;
                    self.aliases.insert(alias, originals[i].to_owned());
                }
            }
            if !parent.is_empty() {
                parent.push('/');
            }
            parent.push_str(component);
            match self.nodes.get_mut(&parent) {
                Some(node) => {
                    if !node.directory || !is_dir || (last && node.explicit) {
                        return Err(ArchiveError::Collision);
                    }
                    if last {
                        node.explicit = true;
                    }
                }
                None => {
                    self.string_bytes += parent.len();
                    self.check_budget()?;
                    self.nodes.insert(
                        parent.clone(),
                        Node {
                            directory: is_dir,
                            explicit: last,
                        },
                    );
                }
            }
        }
        Ok(())
    }

    fn check_budget(&self) -> Result<(), ArchiveError> {
        if self.nodes.len() >= 100_000 || self.string_bytes > 16 * 1024 * 1024 {
            return Err(ArchiveError::Limit("namespace node or string-byte budget"));
        }
        Ok(())
    }
}
