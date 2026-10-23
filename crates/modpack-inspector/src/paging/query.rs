// SPDX-License-Identifier: MIT
use super::*;
impl PreparedInspection {
    pub fn query(&self, query: &InspectionQuery) -> Result<InspectionPage, QueryError> {
        query.validate()?;
        let rows = self.section(query.section);
        let needle = query.search.trim().to_lowercase();
        let mut total = 0u32;
        let mut matched = 0u32;
        let mut page = Vec::new();
        for index in rows {
            let row = &self.rows[*index];
            if query.section == InspectionSection::Overview
                && !query.show_dependencies
                && row.source == RowSource::Dependency
            {
                continue;
            }
            total += 1;
            if !needle.is_empty() && !self.matches(row, &needle) {
                continue;
            }
            if matched >= query.offset && page.len() < query.limit as usize {
                page.push(self.display_row(*index));
            }
            matched += 1;
        }
        if query.offset > matched {
            return Err(QueryError::InvalidRange);
        }
        let end = query.offset + page.len() as u32;
        Ok(InspectionPage {
            query: query.clone(),
            total_rows: total,
            matched_rows: matched,
            next_offset: (end < matched).then_some(end),
            rows: page,
        })
    }

    pub fn detail(&self, query: &InspectionDetailQuery) -> Result<InspectionDetail, QueryError> {
        if !(1..=MAX_DETAIL_BYTES).contains(&query.limit) || query.row_id.len() > 32 {
            return Err(QueryError::InvalidQuery);
        }
        let index = query
            .row_id
            .strip_prefix('r')
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|index| format!("r{index}") == query.row_id)
            .ok_or(QueryError::UnknownRow)?;
        let row = self.rows.get(index).ok_or(QueryError::UnknownRow)?;
        let mut cache = self
            .detail_cache
            .lock()
            .map_err(|_| QueryError::InvalidQuery)?;
        if !cache
            .as_ref()
            .is_some_and(|cached| cached.row == index && cached.field == query.field)
        {
            let text = match query.field {
                DetailField::Path => self.path(row).into_owned(),
                DetailField::Before => {
                    self.value(row, row.before.ok_or(QueryError::UnavailableField)?)
                }
                DetailField::After => {
                    self.value(row, row.after.ok_or(QueryError::UnavailableField)?)
                }
            };
            *cache = Some(DetailCache {
                row: index,
                field: query.field,
                text,
            });
        }
        let text = &cache.as_ref().expect("detail cached").text;
        let start = query.offset as usize;
        if start > text.len() || !text.is_char_boundary(start) {
            return Err(QueryError::InvalidRange);
        }
        let mut end = start.saturating_add(query.limit as usize).min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        if start == end && start < text.len() {
            return Err(QueryError::InvalidRange);
        }
        Ok(InspectionDetail {
            row_id: query.row_id.clone(),
            field: query.field,
            total_bytes: text.len() as u32,
            offset: query.offset,
            next_offset: (end < text.len()).then_some(end as u32),
            text: text[start..end].to_owned(),
        })
    }

    fn matches(&self, row: &Row, needle: &str) -> bool {
        self.path(row).to_lowercase().contains(needle)
            || row.source.label().contains(needle)
            || row
                .change
                .is_some_and(|change| change.label().contains(needle))
            || row
                .before
                .is_some_and(|value| self.value(row, value).to_lowercase().contains(needle))
            || row
                .after
                .is_some_and(|value| self.value(row, value).to_lowercase().contains(needle))
    }
    fn display_row(&self, index: usize) -> InspectionRow {
        let row = &self.rows[index];
        InspectionRow {
            id: format!("r{index}"),
            source: row.source,
            change: row.change,
            path: preview(&self.path(row)),
            before: row.before.map(|value| preview(&self.value(row, value))),
            after: row.after.map(|value| preview(&self.value(row, value))),
        }
    }
}
