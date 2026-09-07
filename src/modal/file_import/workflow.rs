use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    fs::{File, OpenOptions},
    io::BufReader,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use chrono::{DateTime, SubsecRound, Utc};
use exif::{In, Reader, Tag, Value};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::utils::ExifDateTimeExt;

use super::{preview::ImportPreview, preview_tree::PreviewTree};

#[derive(Debug, Error)]
pub enum FileImportWorkflowError {
    #[error("source path is not set")]
    SourcePathNotSet,
    #[error("destination path is not set")]
    DestinationPathNotSet,
    #[error("source path is not a directory")]
    SourcePathNotDirectory,
    #[error("destination path is not a directory")]
    DestinationPathNotDirectory,
    #[error(
        "source and destination directories must not overlap: {} and {}",
        source_path.display(),
        destination_path.display()
    )]
    OverlappingPaths {
        source_path: PathBuf,
        destination_path: PathBuf,
    },
    #[error("destination already contains a file or directory at {}", .0.display())]
    DestinationConflict(PathBuf),
    #[error("unknown subdirectory template variable: {0}")]
    UnknownTemplateVariable(String),
    #[error("invalid date format in subdirectory template: {0}")]
    InvalidDateFormat(String),
    #[error("metadata '{field}' is unavailable for {}", path.display())]
    MetadataUnavailable { field: String, path: PathBuf },
    #[error("subdirectory template produced an invalid relative path: {}", .0.display())]
    InvalidSubdirectory(PathBuf),
    #[error("file operation failed: {0}")]
    IoError(#[from] std::io::Error),
}

pub(super) struct FileImportWorkflow {
    pub(super) source_path: Option<PathBuf>,
    pub(super) destination_path: Option<PathBuf>,
    pub(super) workflow: FileWorkflow,
}

#[derive(Clone)]
pub(super) struct FileWorkflowStepResult {
    /// The source path of the original file being imported.
    pub(super) original_path: PathBuf,
    /// The path produced by the previous workflow step.
    pub(super) output_path: PathBuf,
    /// Filesystem metadata for the original file.
    pub(super) file_metadata: std::fs::Metadata,
    /// EXIF fields used by workflow conditions and templates.
    pub(super) exif: FileExifMetadata,
}

#[derive(Debug, Clone, Default)]
pub(super) struct FileExifMetadata {
    capture_date_time: Option<DateTime<Utc>>,
    iso: Option<u64>,
}

impl FileExifMetadata {
    fn read(path: &Path) -> Self {
        let Ok(file) = File::open(path) else {
            return Self::default();
        };
        let Ok(exif) = Reader::new().read_from_container(&mut BufReader::new(file)) else {
            return Self::default();
        };
        let capture_date_time = exif
            .get_field(Tag::DateTimeOriginal, In::PRIMARY)
            .and_then(|field| match &field.value {
                Value::Ascii(values) => values.first(),
                _ => None,
            })
            .and_then(|value| exif::DateTime::from_ascii(value).ok())
            .and_then(|date_time| date_time.to_chrono_date_time().ok());
        let iso = exif
            .get_field(Tag::PhotographicSensitivity, In::PRIMARY)
            .and_then(|field| field.value.get_uint(0))
            .map(Into::into);

        Self {
            capture_date_time,
            iso,
        }
    }
}

#[derive(Clone)]
pub(super) struct CachedImportFile {
    original_path: PathBuf,
    relative_path: PathBuf,
    file_metadata: std::fs::Metadata,
    exif: FileExifMetadata,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct FileWorkflow {
    pub(super) steps: Vec<WorkflowStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) enum WorkflowStep {
    Filter(Condition),
    AppendSubdirectory(SubdirectoryTemplate),
    Conditional {
        condition: Condition,
        then_workflow: FileWorkflow,
        else_workflow: Option<FileWorkflow>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum MetadataField {
    Extension,
    FileName,
    FileSize,
    ModifiedDateTime,
    CaptureDateTime,
    Iso,
}

impl MetadataField {
    pub(super) const ALL: [Self; 6] = [
        Self::Extension,
        Self::FileName,
        Self::FileSize,
        Self::ModifiedDateTime,
        Self::CaptureDateTime,
        Self::Iso,
    ];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Extension => "Extension",
            Self::FileName => "File name",
            Self::FileSize => "File size",
            Self::ModifiedDateTime => "Modified date/time",
            Self::CaptureDateTime => "Capture date/time",
            Self::Iso => "ISO",
        }
    }

    pub(super) fn default_value(self) -> MetadataValue {
        match self {
            Self::Extension | Self::FileName => MetadataValue::Text(String::new()),
            Self::FileSize | Self::Iso => MetadataValue::Integer(0),
            Self::ModifiedDateTime | Self::CaptureDateTime => {
                MetadataValue::DateTime(Utc::now().trunc_subsecs(0))
            }
        }
    }

    pub(super) fn supported_operators(self) -> &'static [ComparisonOperator] {
        use ComparisonOperator::*;
        match self {
            Self::Extension | Self::FileName => &[Equal],
            Self::FileSize | Self::Iso | Self::ModifiedDateTime | Self::CaptureDateTime => {
                &[Equal, Greater, Less, GreaterOrEqual, LessOrEqual]
            }
        }
    }

    pub(super) fn accepts(self, value: &MetadataValue) -> bool {
        matches!(
            (self, value),
            (Self::Extension | Self::FileName, MetadataValue::Text(_))
                | (Self::FileSize | Self::Iso, MetadataValue::Integer(_))
                | (
                    Self::ModifiedDateTime | Self::CaptureDateTime,
                    MetadataValue::DateTime(_)
                )
        )
    }
}

impl std::fmt::Display for MetadataField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Extension => f.write_str("extension"),
            Self::FileName => f.write_str("file_name"),
            Self::FileSize => f.write_str("file_size"),
            Self::ModifiedDateTime => f.write_str("modified_date_time"),
            Self::CaptureDateTime => f.write_str("capture_date_time"),
            Self::Iso => f.write_str("iso"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum MetadataValue {
    Text(String),
    Integer(u64),
    DateTime(DateTime<Utc>),
}

impl MetadataValue {
    fn summary(&self) -> String {
        match self {
            Self::Text(value) => format!("\"{value}\""),
            Self::Integer(value) => value.to_string(),
            Self::DateTime(value) => value.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) enum Condition {
    Compare {
        field: MetadataField,
        operator: ComparisonOperator,
        value: MetadataValue,
    },
    Exists(MetadataField),
    And(Vec<Condition>),
    Or(Vec<Condition>),
    Not(Box<Condition>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum ComparisonOperator {
    Equal,
    Greater,
    Less,
    GreaterOrEqual,
    LessOrEqual,
}

impl ComparisonOperator {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Equal => "Equals (=)",
            Self::Greater => "Greater than (>)",
            Self::Less => "Less than (<)",
            Self::GreaterOrEqual => "At least (>=)",
            Self::LessOrEqual => "At most (<=)",
        }
    }

    fn matches(self, ordering: Ordering) -> bool {
        match self {
            Self::Equal => ordering.is_eq(),
            Self::Greater => ordering.is_gt(),
            Self::Less => ordering.is_lt(),
            Self::GreaterOrEqual => !ordering.is_lt(),
            Self::LessOrEqual => !ordering.is_gt(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConditionKind {
    Comparison(ComparisonOperator),
    Exists,
    And,
    Or,
    Not,
}

impl ConditionKind {
    pub(super) const ALL: [Self; 9] = [
        Self::Comparison(ComparisonOperator::Equal),
        Self::Comparison(ComparisonOperator::Greater),
        Self::Comparison(ComparisonOperator::Less),
        Self::Comparison(ComparisonOperator::GreaterOrEqual),
        Self::Comparison(ComparisonOperator::LessOrEqual),
        Self::Exists,
        Self::And,
        Self::Or,
        Self::Not,
    ];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Comparison(operator) => operator.label(),
            Self::Exists => "Exists",
            Self::And => "All conditions",
            Self::Or => "Any condition",
            Self::Not => "Not",
        }
    }

    pub(super) fn default_condition(self) -> Condition {
        match self {
            Self::Comparison(operator) => {
                let field = if operator == ComparisonOperator::Equal {
                    MetadataField::Extension
                } else {
                    MetadataField::FileSize
                };
                Condition::Compare {
                    field,
                    operator,
                    value: field.default_value(),
                }
            }
            Self::Exists => Condition::Exists(MetadataField::CaptureDateTime),
            Self::And => Condition::And(vec![Condition::default_equals()]),
            Self::Or => Condition::Or(vec![Condition::default_equals()]),
            Self::Not => Condition::Not(Box::new(Condition::default_equals())),
        }
    }
}

impl Condition {
    pub(super) fn default_equals() -> Self {
        Self::Compare {
            field: MetadataField::Extension,
            operator: ComparisonOperator::Equal,
            value: MetadataValue::Text(String::new()),
        }
    }

    pub(super) fn kind(&self) -> ConditionKind {
        match self {
            Self::Compare { operator, .. } => ConditionKind::Comparison(*operator),
            Self::Exists(_) => ConditionKind::Exists,
            Self::And(_) => ConditionKind::And,
            Self::Or(_) => ConditionKind::Or,
            Self::Not(_) => ConditionKind::Not,
        }
    }

    pub(super) fn summary(&self) -> String {
        match self {
            Self::Compare {
                field,
                operator,
                value,
            } => {
                format!("{} {} {}", field.label(), operator.label(), value.summary())
            }
            Self::Exists(field) => format!("{} exists", field.label()),
            Self::And(conditions) => format!("All of {} conditions", conditions.len()),
            Self::Or(conditions) => format!("Any of {} conditions", conditions.len()),
            Self::Not(condition) => format!("Not ({})", condition.summary()),
        }
    }

    fn evaluate(&self, result: &FileWorkflowStepResult) -> bool {
        match self {
            Self::Compare {
                field,
                operator,
                value,
            } => {
                let Some(actual) = result.metadata_value(*field) else {
                    return false;
                };
                let ordering = match (&actual, value) {
                    (MetadataValue::Text(actual), MetadataValue::Text(expected)) => {
                        if *field == MetadataField::Extension {
                            actual
                                .to_ascii_lowercase()
                                .cmp(&expected.trim_start_matches('.').to_ascii_lowercase())
                        } else {
                            actual.cmp(expected)
                        }
                    }
                    (MetadataValue::Integer(actual), MetadataValue::Integer(expected)) => {
                        actual.cmp(expected)
                    }
                    (MetadataValue::DateTime(actual), MetadataValue::DateTime(expected)) => {
                        actual.cmp(expected)
                    }
                    _ => return false,
                };
                operator.matches(ordering)
            }
            Self::Exists(field) => result.metadata_value(*field).is_some(),
            Self::And(conditions) => conditions
                .iter()
                .all(|condition| condition.evaluate(result)),
            Self::Or(conditions) => conditions
                .iter()
                .any(|condition| condition.evaluate(result)),
            Self::Not(condition) => !condition.evaluate(result),
        }
    }
}

impl FileWorkflowStepResult {
    fn metadata_value(&self, field: MetadataField) -> Option<MetadataValue> {
        match field {
            MetadataField::Extension => self
                .output_path
                .extension()
                .map(|extension| MetadataValue::Text(extension.to_string_lossy().to_string())),
            MetadataField::FileName => self
                .output_path
                .file_name()
                .map(|file_name| MetadataValue::Text(file_name.to_string_lossy().to_string())),
            MetadataField::FileSize => Some(MetadataValue::Integer(self.file_metadata.len())),
            MetadataField::ModifiedDateTime => self
                .file_metadata
                .modified()
                .ok()
                .map(|date_time| MetadataValue::DateTime(date_time.into())),
            MetadataField::CaptureDateTime => {
                self.exif.capture_date_time.map(MetadataValue::DateTime)
            }
            MetadataField::Iso => self.exif.iso.map(MetadataValue::Integer),
        }
    }
}

/// A relative directory template. Supported variables include `{{iso}}`,
/// `{{extension}}`, `{{file_name}}`, `{{file_size}}`, `{{capture_date}}`, and
/// `{{modified_date}}`. Date variables accept a chrono format after a colon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct SubdirectoryTemplate {
    pub(super) template: String,
}

impl SubdirectoryTemplate {
    fn render(&self, result: &FileWorkflowStepResult) -> Result<PathBuf, FileImportWorkflowError> {
        let mut rendered = String::new();
        let mut remaining = self.template.as_str();

        while let Some(start) = remaining.find("{{") {
            rendered.push_str(&remaining[..start]);
            let variable_start = start + 2;
            let Some(end_offset) = remaining[variable_start..].find("}}") else {
                return Err(FileImportWorkflowError::UnknownTemplateVariable(
                    remaining[variable_start..].to_string(),
                ));
            };
            let end = variable_start + end_offset;
            let variable = remaining[variable_start..end].trim();
            rendered.push_str(&Self::render_variable(variable, result)?);
            remaining = &remaining[end + 2..];
        }
        rendered.push_str(remaining);

        let path = PathBuf::from(rendered);
        if path.as_os_str().is_empty()
            || path.is_absolute()
            || path.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return Err(FileImportWorkflowError::InvalidSubdirectory(path));
        }

        Ok(path)
    }

    fn render_variable(
        variable: &str,
        result: &FileWorkflowStepResult,
    ) -> Result<String, FileImportWorkflowError> {
        let metadata = |field: MetadataField| {
            result.metadata_value(field).ok_or_else(|| {
                FileImportWorkflowError::MetadataUnavailable {
                    field: field.to_string(),
                    path: result.original_path.clone(),
                }
            })
        };

        match variable {
            "extension" => match metadata(MetadataField::Extension)? {
                MetadataValue::Text(value) => Ok(value),
                _ => unreachable!(),
            },
            "file_name" => match metadata(MetadataField::FileName)? {
                MetadataValue::Text(value) => Ok(value),
                _ => unreachable!(),
            },
            "file_size" => match metadata(MetadataField::FileSize)? {
                MetadataValue::Integer(value) => Ok(value.to_string()),
                _ => unreachable!(),
            },
            "iso" => match metadata(MetadataField::Iso)? {
                MetadataValue::Integer(value) => Ok(value.to_string()),
                _ => unreachable!(),
            },
            "date" | "capture_date" => match metadata(MetadataField::CaptureDateTime)? {
                MetadataValue::DateTime(value) => Ok(value.format("%Y-%m-%d").to_string()),
                _ => unreachable!(),
            },
            variable if variable.starts_with("date:") || variable.starts_with("capture_date:") => {
                let format = variable.split_once(':').map_or("", |(_, format)| format);
                match metadata(MetadataField::CaptureDateTime)? {
                    MetadataValue::DateTime(value) => Self::format_date(value, format),
                    _ => unreachable!(),
                }
            }
            "modified_date" => match metadata(MetadataField::ModifiedDateTime)? {
                MetadataValue::DateTime(value) => Ok(value.format("%Y-%m-%d").to_string()),
                _ => unreachable!(),
            },
            variable if variable.starts_with("modified_date:") => {
                match metadata(MetadataField::ModifiedDateTime)? {
                    MetadataValue::DateTime(value) => Self::format_date(value, &variable[14..]),
                    _ => unreachable!(),
                }
            }
            _ => Err(FileImportWorkflowError::UnknownTemplateVariable(
                variable.to_string(),
            )),
        }
    }

    fn format_date(value: DateTime<Utc>, format: &str) -> Result<String, FileImportWorkflowError> {
        let mut rendered = String::new();
        // User-entered chrono formats can fail to format. `to_string()` would panic.
        std::fmt::write(&mut rendered, format_args!("{}", value.format(format)))
            .map_err(|_| FileImportWorkflowError::InvalidDateFormat(format.to_owned()))?;
        Ok(rendered)
    }
}

impl FileWorkflow {
    fn execute(
        &self,
        mut result: FileWorkflowStepResult,
    ) -> Result<Option<FileWorkflowStepResult>, FileImportWorkflowError> {
        for step in &self.steps {
            result = match step.execute(result)? {
                Some(result) => result,
                None => return Ok(None),
            };
        }
        Ok(Some(result))
    }
}

impl WorkflowStep {
    fn execute(
        &self,
        mut result: FileWorkflowStepResult,
    ) -> Result<Option<FileWorkflowStepResult>, FileImportWorkflowError> {
        match self {
            Self::Filter(condition) => Ok(condition.evaluate(&result).then_some(result)),
            Self::AppendSubdirectory(template) => {
                let subdirectory = template.render(&result)?;
                if let Some(file_name) = result.output_path.file_name().map(ToOwned::to_owned) {
                    let parent = result.output_path.parent().unwrap_or(Path::new(""));
                    result.output_path = parent.join(subdirectory).join(file_name);
                }
                Ok(Some(result))
            }
            Self::Conditional {
                condition,
                then_workflow,
                else_workflow,
            } => {
                if condition.evaluate(&result) {
                    then_workflow.execute(result)
                } else if let Some(else_workflow) = else_workflow {
                    else_workflow.execute(result)
                } else {
                    Ok(Some(result))
                }
            }
        }
    }
}

impl FileImportWorkflow {
    pub(super) fn new(source_path: Option<PathBuf>, destination_path: Option<PathBuf>) -> Self {
        Self {
            source_path,
            destination_path,
            workflow: FileWorkflow::default(),
        }
    }

    pub(super) fn run(&self) -> Result<(), FileImportWorkflowError> {
        let (_, destination_path) = self.validate_paths()?;
        let files = self.scan_source()?;
        let results = self.evaluate(&files, destination_path)?;
        if let Some(conflict) = Self::destination_conflicts(&results, destination_path)
            .into_iter()
            .next()
        {
            return Err(FileImportWorkflowError::DestinationConflict(conflict));
        }
        for result in results {
            Self::copy(&result)?;
        }
        Ok(())
    }

    fn validate_paths(&self) -> Result<(&Path, &Path), FileImportWorkflowError> {
        let source_path = self
            .source_path
            .as_deref()
            .ok_or(FileImportWorkflowError::SourcePathNotSet)?;
        let destination_path = self
            .destination_path
            .as_deref()
            .ok_or(FileImportWorkflowError::DestinationPathNotSet)?;

        if !source_path.is_dir() {
            return Err(FileImportWorkflowError::SourcePathNotDirectory);
        }
        if !destination_path.is_dir() {
            return Err(FileImportWorkflowError::DestinationPathNotDirectory);
        }
        let canonical_source = source_path.canonicalize()?;
        let canonical_destination = destination_path.canonicalize()?;
        if canonical_source.starts_with(&canonical_destination)
            || canonical_destination.starts_with(&canonical_source)
        {
            return Err(FileImportWorkflowError::OverlappingPaths {
                source_path: source_path.to_path_buf(),
                destination_path: destination_path.to_path_buf(),
            });
        }
        Ok((source_path, destination_path))
    }

    pub(super) fn scan_source(&self) -> Result<Vec<CachedImportFile>, FileImportWorkflowError> {
        let (source_path, _) = self.validate_paths()?;
        let mut results = Vec::new();
        Self::collect_file_metadata(source_path, source_path, &mut results)?;
        Ok(results)
    }

    fn evaluate(
        &self,
        files: &[CachedImportFile],
        destination_path: &Path,
    ) -> Result<Vec<FileWorkflowStepResult>, FileImportWorkflowError> {
        let mut results = Vec::new();
        for file in files {
            let result = FileWorkflowStepResult {
                original_path: file.original_path.clone(),
                output_path: destination_path.join(&file.relative_path),
                file_metadata: file.file_metadata.clone(),
                exif: file.exif.clone(),
            };
            if let Some(result) = self.workflow.execute(result)? {
                results.push(result);
            }
        }
        Ok(results)
    }

    pub(super) fn build_preview(
        &self,
        files: &[CachedImportFile],
    ) -> Result<ImportPreview, FileImportWorkflowError> {
        let (_, destination_path) = self.validate_paths()?;
        let results = self.evaluate(files, destination_path)?;
        let conflicts = Self::destination_conflicts(&results, destination_path);
        let mut source_by_output = HashMap::new();
        let virtual_root = destination_path
            .file_name()
            .filter(|name| !name.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("Destination"));
        let mut tree = PreviewTree::default();

        for result in &results {
            let relative_output = result
                .output_path
                .strip_prefix(destination_path)
                .unwrap_or(&result.output_path);
            let virtual_output = virtual_root.join(relative_output);
            tree.insert(relative_output);
            source_by_output.insert(virtual_output, result.original_path.clone());
        }

        let items = tree.into_items(virtual_root);

        Ok(ImportPreview {
            items: Arc::new(items),
            source_by_output: Arc::new(source_by_output),
            conflicts: Arc::new(conflicts),
            imported_count: results.len(),
            dropped_count: files.len().saturating_sub(results.len()),
        })
    }

    fn destination_conflicts(
        results: &[FileWorkflowStepResult],
        destination_root: &Path,
    ) -> Vec<PathBuf> {
        let mut planned_files = HashSet::new();
        let mut planned_directories = HashSet::new();
        let mut conflicts = Vec::new();
        for result in results {
            let output = &result.output_path;
            let collision_key = Self::path_collision_key(output);
            // Check ancestors rather than comparing every pair of planned files.
            let planned_collision = planned_directories.contains(&collision_key)
                || collision_key
                    .ancestors()
                    .any(|ancestor| planned_files.contains(ancestor));
            planned_directories.extend(collision_key.ancestors().skip(1).map(Path::to_path_buf));
            planned_files.insert(collision_key);
            let Some(relative_output) = output.strip_prefix(destination_root).ok() else {
                conflicts.push(output.clone());
                continue;
            };

            let mut candidate = destination_root.to_path_buf();
            let components = relative_output.components().collect::<Vec<_>>();
            let mut filesystem_conflict = None;
            for (index, component) in components.iter().enumerate() {
                candidate.push(component.as_os_str());
                match std::fs::symlink_metadata(&candidate) {
                    Ok(metadata) => {
                        let is_output = index + 1 == components.len();
                        if metadata.file_type().is_symlink() || is_output || !metadata.is_dir() {
                            filesystem_conflict = Some(candidate.clone());
                            break;
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(_) => {
                        filesystem_conflict = Some(candidate.clone());
                        break;
                    }
                }
            }
            if planned_collision {
                conflicts.push(output.clone());
            }
            if let Some(conflict) = filesystem_conflict {
                conflicts.push(conflict);
            }
        }
        conflicts.sort();
        conflicts.dedup();
        conflicts
    }

    fn path_collision_key(path: &Path) -> PathBuf {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            PathBuf::from(path.to_string_lossy().to_lowercase())
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            path.to_path_buf()
        }
    }

    pub(super) fn copy(result: &FileWorkflowStepResult) -> Result<(), std::io::Error> {
        if let Some(parent) = result.output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut source = File::open(&result.original_path)?;
        let mut destination = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&result.output_path)?;
        if let Err(error) = std::io::copy(&mut source, &mut destination) {
            drop(destination);
            let _ = std::fs::remove_file(&result.output_path);
            return Err(error);
        }
        if let Err(error) =
            std::fs::set_permissions(&result.output_path, result.file_metadata.permissions())
        {
            let _ = std::fs::remove_file(&result.output_path);
            return Err(error);
        }
        Ok(())
    }

    fn collect_file_metadata(
        source_root: &Path,
        directory: &Path,
        results: &mut Vec<CachedImportFile>,
    ) -> Result<(), std::io::Error> {
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let source_path = entry.path();

            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                Self::collect_file_metadata(source_root, &source_path, results)?;
            } else if file_type.is_file() {
                results.push(CachedImportFile {
                    relative_path: source_path
                        .strip_prefix(source_root)
                        .unwrap_or(&source_path)
                        .to_path_buf(),
                    exif: FileExifMetadata::read(&source_path),
                    original_path: source_path,
                    file_metadata: entry.metadata()?,
                });
            }
        }

        Ok(())
    }
}
