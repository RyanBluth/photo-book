use std::{
    collections::HashSet,
    fmt::{self, Display},
    iter::Peekable,
    path::{Components, Path, PathBuf},
    sync::Arc,
};

use log::warn;

/// Represents a node in a file system tree structure.
///
/// A node can either be a file or a directory. Directories can contain other nodes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileTreeNode {
    /// A directory node, containing its path and a list of child nodes.
    Directory(PathBuf, Vec<FileTreeNode>),
    /// A file node, containing its path.
    File(PathBuf),
}

impl FileTreeNode {
    /// Returns the path of this `FileTreeNode`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::path::PathBuf;
    /// # use photobook_rs::file_tree::FileTreeNode; // Assuming this is the correct path to your type
    /// let file_node = FileTreeNode::File(PathBuf::from("example.txt"));
    /// assert_eq!(file_node.path(), &PathBuf::from("example.txt"));
    ///
    /// let dir_node = FileTreeNode::Directory(PathBuf::from("my_dir"), vec![]);
    /// assert_eq!(dir_node.path(), &PathBuf::from("my_dir"));
    /// ```
    pub fn path(&self) -> &PathBuf {
        match self {
            FileTreeNode::Directory(p, _) => p,
            FileTreeNode::File(p) => p,
        }
    }

    /// Recursively builds a `FileTreeNode` structure from a path's components.
    ///
    /// This is an internal helper function used by the `From<&Path>` implementation
    /// for `FileTreeNode`.
    ///
    /// # Arguments
    ///
    /// * `current_node_full_path`: The full path accumulated so far for the current node being built.
    /// * `remaining_child_components_iter`: A peekable iterator over the remaining path components
    ///   that form the children of the `current_node_full_path`.
    /// * `original_input_path`: The complete original path that was used to start the build process.
    ///   This is used to determine if the final leaf node should be a `File` or a `Directory`,
    ///   based on the nature of the `original_input_path` itself.
    fn build_recursive(
        current_node_full_path: PathBuf,
        mut remaining_child_components_iter: Peekable<Components<'_>>,
        original_input_path: &Path,
    ) -> Self {
        match remaining_child_components_iter.next() {
            None => {
                // If there are no more components, this node is the final one.
                // Its type (File/Directory) depends on the original input path's type.
                if original_input_path.is_dir() {
                    FileTreeNode::Directory(current_node_full_path, Vec::new())
                } else {
                    FileTreeNode::File(current_node_full_path)
                }
            }
            Some(child_component) => {
                // There are more components; this node is a directory.
                // Create the child's full path and recurse.
                let mut child_node_full_path = current_node_full_path.clone();
                child_node_full_path.push(child_component.as_os_str());

                FileTreeNode::Directory(
                    current_node_full_path,
                    vec![Self::build_recursive(
                        child_node_full_path,
                        remaining_child_components_iter,
                        original_input_path,
                    )],
                )
            }
        }
    }

    /// Merges another `FileTreeNode` into this one.
    ///
    /// This method is typically used to combine file system structures. For example,
    /// if `self` represents `/a/b` and `other` represents `/a/c`, merging `other`
    /// into a tree rooted at `/a` would add `c` as a child of `b`'s parent (`a`).
    ///
    /// The core logic handles several cases:
    /// * **Directory into Directory**: If both nodes are directories and their paths match,
    ///   their children are merged. Children from `other` are recursively merged into
    ///   matching children in `self`, or added if no match exists.
    /// * **File into File**: If both nodes are files and their paths match, no structural
    ///   change occurs. This operation is essentially a no-op for file content but
    ///   confirms path consistency.
    /// * **Directory into File / File into Directory**: If paths match but types differ,
    ///   `self` is replaced by `other`. This allows a path previously thought to be a file
    ///   to become a directory (and vice-versa), reflecting updates in the file system model.
    ///
    /// If the paths of `self` and `other` do not match at the point of merging, a warning
    /// is logged, and the merge operation for that specific branch might be incomplete or aborted.
    ///
    /// This method consumes `other`.
    pub fn merge(&mut self, other: FileTreeNode) {
        let self_original_path = self.path().clone(); // Path of `self` before any potential mutation.

        match (self, other) {
            // Case 1: Both are Directories. Merge children.
            (
                FileTreeNode::Directory(_, self_children_vec),
                FileTreeNode::Directory(other_path_d, other_children_to_add_vec),
            ) => {
                if self_original_path != other_path_d {
                    warn!(
                        "Path mismatch in merge (Dir, Dir): self='{:?}', other='{:?}'. Merge may be incomplete.",
                        self_original_path, other_path_d
                    );
                    return;
                }

                for other_child in other_children_to_add_vec {
                    let other_child_path_target = other_child.path();
                    match self_children_vec.iter_mut().find(|existing_self_child| {
                        existing_self_child.path() == other_child_path_target
                    }) {
                        Some(found_self_child_to_recurse_into) => {
                            found_self_child_to_recurse_into.merge(other_child);
                        }
                        None => {
                            self_children_vec.push(other_child);
                        }
                    }
                }
            }

            // Case 2: Both are Files. No structural change if paths match.
            (
                FileTreeNode::File(_), // Path of self is `self_original_path`.
                FileTreeNode::File(other_path_f),
            ) => {
                if self_original_path != other_path_f {
                    warn!(
                        "Path mismatch in merge (File, File): self='{:?}', other='{:?}'",
                        self_original_path, other_path_f
                    );
                }
                // If paths match, no structural change. `other_path_f` is dropped. `other` is consumed.
            }

            // Case 3: Existing is Directory, incoming is File. Replace self if paths match.
            (
                self_node_being_replaced @ FileTreeNode::Directory(_, _),
                FileTreeNode::File(other_path_f_owned),
            ) => {
                if self_original_path == other_path_f_owned {
                    *self_node_being_replaced = FileTreeNode::File(other_path_f_owned);
                } else {
                    warn!(
                        "Path mismatch in merge (Dir -> File): self='{:?}', other='{:?}'",
                        self_original_path, other_path_f_owned
                    );
                }
            }

            // Case 4: Existing is File, incoming is Directory. Replace self if paths match.
            (
                self_node_being_replaced @ FileTreeNode::File(_),
                FileTreeNode::Directory(other_path_d_owned, other_children_owned),
            ) => {
                if self_original_path == other_path_d_owned {
                    *self_node_being_replaced =
                        FileTreeNode::Directory(other_path_d_owned, other_children_owned);
                } else {
                    warn!(
                        "Path mismatch in merge (File -> Dir): self='{:?}', other='{:?}'",
                        self_original_path, other_path_d_owned
                    );
                }
            }
        }
    }

    /// Removes a path from this `FileTreeNode`.
    ///
    /// This method removes the specified path from the tree structure. If the path
    /// corresponds to a file, it is removed from its parent directory. If the path
    /// corresponds to a directory, the entire directory and its contents are removed.
    ///
    /// # Returns
    /// * `true` if the path was found and removed
    /// * `false` if the path was not found
    pub fn remove(&mut self, path_to_remove: &Path) -> bool {
        match self {
            FileTreeNode::File(file_path) => {
                // If this is the file to remove, we can't remove ourselves directly
                // The parent needs to handle this
                file_path == path_to_remove
            }
            FileTreeNode::Directory(dir_path, children) => {
                if dir_path == path_to_remove {
                    // If this directory is the one to remove, we can't remove ourselves
                    // The parent needs to handle this
                    return true;
                }

                // Check if any child matches the path to remove
                let mut found_index = None;
                for (i, child) in children.iter_mut().enumerate() {
                    if child.path() == path_to_remove {
                        found_index = Some(i);
                        break;
                    } else if child.remove(path_to_remove) {
                        // Child found and removed the path, or child itself should be removed
                        if child.path() == path_to_remove {
                            found_index = Some(i);
                        }
                        break;
                    }
                }

                if let Some(index) = found_index {
                    children.remove(index);
                    return true;
                }

                false
            }
        }
    }

    /// Recursive helper for displaying the `FileTreeNode` with indentation.
    /// Used by the `Display` implementation for `FileTree`.
    fn fmt_recursive(
        &self,
        f: &mut fmt::Formatter<'_>,
        indent_level: usize,
        is_last_child: bool,
    ) -> fmt::Result {
        let indent_prefix = if indent_level > 0 {
            "│   ".repeat(indent_level.saturating_sub(1))
                + if is_last_child {
                    "└── "
                } else {
                    "├── "
                }
        } else {
            "".to_string()
        };

        let display_name = self
            .path()
            .file_name()
            .unwrap_or_else(|| self.path().as_os_str()) // Fallback for root paths like "/" or "."
            .to_string_lossy();

        writeln!(f, "{}{}", indent_prefix, display_name)?;

        if let FileTreeNode::Directory(_, children) = self {
            let mut sorted_children = children.clone(); // Clone for sorting; display only
            sorted_children.sort_by_key(|child| child.path().clone());

            let num_children = sorted_children.len();
            for (i, child) in sorted_children.iter().enumerate() {
                child.fmt_recursive(f, indent_level + 1, i == num_children - 1)?;
            }
        }
        fmt::Result::Ok(())
    }
}

/// Creates a `FileTreeNode` from a `&Path`.
///
/// The resulting node structure reflects the hierarchy of the input path.
/// For example, `Path::new("a/b/c.txt")` would create a nested structure of
/// directories 'a' and 'b', with 'c.txt' as a file node under 'b'.
/// The type of the final node (File or Directory) is determined by `original_input_path.is_dir()`.
impl From<&Path> for FileTreeNode {
    fn from(full_path: &Path) -> Self {
        let mut components_iter = full_path.components().peekable();

        match components_iter.next() {
            Some(first_component) => {
                let initial_node_full_path = PathBuf::from(first_component.as_os_str());
                FileTreeNode::build_recursive(initial_node_full_path, components_iter, full_path)
            }
            None => {
                // Handles cases like Path::new("") or Path::new(".")
                if full_path.is_dir() {
                    // Check if an empty-component path refers to a directory (e.g. ".")
                    FileTreeNode::Directory(full_path.to_path_buf(), Vec::new())
                } else {
                    // Typically for Path::new("") which is not a dir, or a file like ".config"
                    FileTreeNode::File(full_path.to_path_buf())
                }
            }
        }
    }
}

/// Displays the last component of the node's path (its name).
///
/// For a full, structured display of a tree, use the `Display` implementation
/// on `FileTree` or `FileTreeCollection`.
impl Display for FileTreeNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let display_name = self
            .path()
            .file_name()
            .unwrap_or_else(|| self.path().as_os_str()) // Fallback for paths like "/" or "."
            .to_string_lossy();
        write!(f, "{}", display_name)
    }
}

/// Represents a single file system tree, with a designated root node.
///
/// A `FileTree` encapsulates a `FileTreeNode` that acts as the root of this particular tree structure.
/// It provides methods to initialize the tree from a root path and to insert new paths into it.
#[derive(Clone, Debug)]
pub struct FileTree {
    /// The root node of this file tree.
    pub root: FileTreeNode,
}

impl FileTree {
    /// Creates a new `FileTree` rooted at the given `root_path`.
    ///
    /// The `root_path` itself becomes the root node of this tree. The structure of this root node
    /// (i.e., whether it's a File or Directory and its potential children if the path has multiple
    /// components) is determined by the `FileTreeNode::from(root_path)` conversion.
    ///
    /// To represent an empty tree structure that can be populated later (e.g., in a `FileTreeCollection`
    /// before any specific root component is known), you can use a conceptual empty path like `Path::new("")`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::path::Path;
    /// # use photobook_rs::file_tree::{FileTree, FileTreeNode};
    /// let tree = FileTree::new(Path::new("my_project/src/main.rs"));
    /// // tree.root will be a Directory "my_project" containing Directory "src" containing File "main.rs"
    ///
    /// let empty_root_tree = FileTree::new(Path::new(""));
    /// // empty_root_tree.root might be FileTreeNode::File("") or Directory("") depending on Path::new("").is_dir()
    /// ```
    pub fn new(root_path: &Path) -> Self {
        FileTree {
            root: FileTreeNode::from(root_path),
        }
    }

    /// Inserts a path into the `FileTree`.
    ///
    /// The behavior of this method depends on the relationship between the `FileTree`'s current root
    /// and the root of the `path_to_insert` (derived from `FileTreeNode::from(path_to_insert)`).
    ///
    /// * **Matching Roots**: If the path of `self.root` is identical to the path of the node derived from
    ///   `path_to_insert`, the `new_branch_root_node` is merged into `self.root` using `FileTreeNode::merge`.
    ///   This is the common case for adding files/directories to an existing, correctly-rooted tree.
    ///
    /// * **Placeholder Root Initialization**: If `self.root` represents a placeholder (e.g., created from `Path::new("")`
    ///   and is currently a `FileTreeNode::File("")`) and `path_to_insert` is a non-empty path, `self.root`
    ///   is replaced by the `new_branch_root_node`. This allows a `FileTree` initially created with an empty
    ///   root to be properly initialized with its first actual path.
    ///
    /// * **Mismatched Roots**: Other scenarios typically indicate a logical error or an attempt to insert
    ///   a path that doesn't belong in this tree:
    ///     * Inserting an empty path into an established tree (non-empty root).
    ///     * Inserting a path whose root component differs from the tree's existing, non-empty root.
    ///   In these cases, a warning is logged, and the path is not inserted.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::path::Path;
    /// # use photobook_rs::file_tree::FileTree;
    /// // Create a tree rooted at "project_alpha"
    /// let mut tree = FileTree::new(Path::new("project_alpha"));
    ///
    /// // Insert a file within this project
    /// tree.insert(Path::new("project_alpha/src/main.rs"));
    ///
    /// // Insert another file
    /// tree.insert(Path::new("project_alpha/README.md"));
    ///
    /// // Attempting to insert a path with a different root will typically be a no-op (with a warning)
    /// tree.insert(Path::new("project_beta/config.toml")); // project_beta != project_alpha
    /// ```
    pub fn insert(&mut self, path_to_insert: &Path) {
        let new_branch_root_node = FileTreeNode::from(path_to_insert);

        if self.root.path() == new_branch_root_node.path() {
            // Handles cases where roots match, including if both are, e.g., from Path::new("").
            self.root.merge(new_branch_root_node);
            return;
        }

        // At this point, root paths are different.
        let self_root_path = self.root.path();
        let new_branch_root_path = new_branch_root_node.path();

        let self_is_placeholder_empty_file =
            self_root_path.as_os_str().is_empty() && matches!(self.root, FileTreeNode::File(_));
        let new_branch_is_empty_path = new_branch_root_path.as_os_str().is_empty();

        if self_is_placeholder_empty_file && !new_branch_is_empty_path {
            // Case: Initializing a tree that was a placeholder (e.g. from FileTree::new(Path::new("")))
            // with the first real path. The new branch becomes the tree's root.
            self.root = new_branch_root_node;
        } else if !self_root_path.as_os_str().is_empty() && new_branch_is_empty_path {
            // Case: Trying to insert an empty path (e.g. Path::new("")) into an established tree.
            warn!(
                "Attempted to insert an empty path into an established tree. Insert ignored. Tree root: {:?}, New branch root: {:?}",
                self_root_path, new_branch_root_path
            );
        } else {
            // Case: Root paths differ and are non-empty, or other unhandled scenarios.
            warn!(
                "Root path mismatch during insert. Tree root: {:?}, New branch root: {:?}. Path not inserted.",
                self_root_path, new_branch_root_path
            );
        }
    }

    /// Removes a path from the `FileTree`.
    ///
    /// This method removes the specified path from the tree structure. If the path
    /// is found, it will be removed from the tree.
    ///
    /// # Returns
    /// * `true` if the path was found and removed
    /// * `false` if the path was not found
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::path::Path;
    /// # use photobook_rs::file_tree::FileTree;
    /// let mut tree = FileTree::new(Path::new("project"));
    /// tree.insert(Path::new("project/src/main.rs"));
    ///
    /// // Remove the file
    /// let removed = tree.remove(Path::new("project/src/main.rs"));
    /// assert!(removed);
    /// ```
    pub fn remove(&mut self, path_to_remove: &Path) -> bool {
        if self.root.path() == path_to_remove {
            // Cannot remove the root itself, but we can indicate it was found
            return true;
        }

        self.root.remove(path_to_remove)
    }
}

/// Implements `Display` for `FileTree` to print a human-readable, nested structure of the tree.
///
/// The output starts with the root node's name, followed by its children, indented appropriately.
/// Files and directories are sorted alphabetically at each level for consistent display.
///
/// # Example Output
///
/// ```text
/// my_project
/// ├── README.md
/// └── src
///     ├── lib.rs
///     └── main.rs
/// ```
impl Display for FileTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let root_display_name = self
            .root
            .path()
            .file_name()
            .unwrap_or_else(|| self.root.path().as_os_str()) // Fallback for root paths like "/"
            .to_string_lossy();

        writeln!(f, "{}", root_display_name)?;

        if let FileTreeNode::Directory(_, children) = &self.root {
            let mut sorted_children = children.clone(); // Clone for sorting; display only
            sorted_children.sort_by_key(|child| child.path().clone());

            let num_children = sorted_children.len();
            for (i, child) in sorted_children.iter().enumerate() {
                // Children are displayed with an initial indent level of 1.
                child.fmt_recursive(f, 1, i == num_children - 1)?;
            }
        }
        fmt::Result::Ok(())
    }
}

/// Represents a node from a `FileTreeCollection` when iterated over in a flattened manner.
///
/// This struct pairs a `FileTreeNode` with its `depth` in the overall tree structure
/// (where root nodes of individual trees in the collection are at depth 0).
#[derive(Debug, Clone)]
pub struct FlattenedTreeItem {
    pub is_root: bool,
    /// The file tree node itself.
    pub node: FileTreeNode,
    /// The depth of this node in the tree structure. Root nodes of trees within
    /// a `FileTreeCollection` are at depth 0, their direct children at depth 1, and so on.
    pub depth: usize,
}

/// An iterator for traversing a `FileTreeCollection` in a depth-first manner.
///
/// This iterator yields `FlattenedTreeItem` instances, allowing consumption of the tree
/// structure as a linear sequence of nodes, each tagged with its depth.
/// The iteration proceeds by visiting a node, then its children (recursively), before
/// moving to its siblings.
pub struct FileTreeIterator {
    /// Stack of nodes to process: (node, depth).
    /// Nodes are pushed onto the stack and popped off, with children being added
    /// in reverse order to ensure correct depth-first traversal (left-most child first).
    stack: Vec<(FileTreeNode, usize)>,
}

impl FileTreeIterator {
    /// Creates a new FileTreeIterator
    pub fn new(stack: Vec<(FileTreeNode, usize)>) -> Self {
        Self { stack }
    }

    /// Attempts to compress a directory chain if it has a series of single-child directories
    fn compress_directory_chain(
        &self,
        dir_node: &FileTreeNode,
        depth: usize,
        is_root: bool,
    ) -> Option<(FlattenedTreeItem, Vec<FileTreeNode>)> {
        match dir_node {
            FileTreeNode::Directory(dir_path, children) => {
                if children.len() != 1 {
                    return None; // Not a candidate for compression
                }

                // Check if the single child is a directory
                match &children[0] {
                    FileTreeNode::Directory(_, _) => {
                        // Start collecting the chain
                        let mut skipped_nodes = Vec::new();
                        let mut full_path = dir_path.clone();

                        // Traverse the chain, collecting single-child directories
                        let mut current = &children[0];
                        skipped_nodes.push(current.clone());

                        loop {
                            match current {
                                FileTreeNode::Directory(child_path, child_children) => {
                                    // If this directory has exactly one child directory, continue the chain
                                    if child_children.len() == 1 {
                                        match &child_children[0] {
                                            FileTreeNode::Directory(_, _) => {
                                                current = &child_children[0];
                                                skipped_nodes.push(current.clone());
                                                continue;
                                            }
                                            _ => break, // Child is not a directory, end the chain
                                        }
                                    }
                                    break; // Multiple children or no children, end the chain
                                }
                                _ => break, // Not a directory, end the chain (shouldn't happen)
                            }
                        }

                        // If we collected multiple nodes, create a compressed item
                        if skipped_nodes.len() > 0 {
                            // Get the last node to find its children
                            if let Some(last_node) = skipped_nodes.last() {
                                if let FileTreeNode::Directory(last_path, last_children) = last_node
                                {
                                    // Create a new path that combines all paths in the chain
                                    for node in &skipped_nodes {
                                        if let FileTreeNode::Directory(path, _) = node {
                                            if let Some(file_name) = path.file_name() {
                                                full_path.push(file_name);
                                            }
                                        }
                                    }

                                    // Create a new directory node with the full path and the last node's children
                                    let new_dir_node =
                                        FileTreeNode::Directory(full_path, last_children.clone());

                                    return Some((
                                        FlattenedTreeItem {
                                            is_root: is_root,
                                            node: new_dir_node,
                                            depth,
                                        },
                                        skipped_nodes,
                                    ));
                                }
                            }
                        }
                    }
                    _ => {} // Child is not a directory, can't compress
                }
            }
            _ => {} // Not a directory, can't compress
        }

        None
    }
}

/// Enables iteration over a `&FileTreeCollection` using a `for` loop.
///
/// The iteration produces `FlattenedTreeItem`s, representing each node in the
/// collection along with its depth. Traversal is depth-first.
impl IntoIterator for &FileTreeCollection {
    type Item = FlattenedTreeItem;
    type IntoIter = FileTreeIterator;

    /// Creates a `FileTreeIterator` for the given `FileTreeCollection`.
    ///
    /// Initializes the iterator by pushing the root nodes of all trees in the collection
    /// onto its internal stack. The roots are pushed in reverse order of their appearance
    /// in the collection to ensure that the first tree's root is processed first.
    fn into_iter(self) -> Self::IntoIter {
        let mut stack = Vec::new();
        // Clone file_trees for iteration to avoid borrowing issues if self is modified elsewhere.
        // This iterator is over a snapshot.
        let file_trees = self.file_trees.clone();

        // Add root nodes of all trees to the stack, in reverse order
        // so they are popped in the original order.
        for tree in file_trees.iter().rev() {
            stack.push((tree.root.clone(), 0)); // Depth 0 for roots of each tree in collection
        }

        FileTreeIterator::new(stack)
    }
}

/// Implements the iteration logic for `FileTreeIterator`.
impl Iterator for FileTreeIterator {
    type Item = FlattenedTreeItem;

    /// Advances the iterator and returns the next `FlattenedTreeItem` in depth-first order.
    ///
    /// When a `FileTreeNode::Directory` is popped from the stack, its children (if any)
    /// are pushed onto the stack in reverse order before the directory itself is returned.
    /// This ensures that children are visited before siblings and in their natural order.
    /// Returns `None` when all nodes in the collection have been visited.
    fn next(&mut self) -> Option<Self::Item> {
        if let Some((current_node, depth)) = self.stack.pop() {
            // Check if we can compress this directory chain
            if let Some((compressed_item, skipped_nodes)) =
                self.compress_directory_chain(&current_node, depth, depth == 0)
            {
                // Get the last node in the chain to find its children
                if let Some(last_node) = skipped_nodes.last() {
                    if let FileTreeNode::Directory(_, last_children) = last_node {
                        // Add the children of the last node in the chain to the stack
                        for child in last_children.iter().rev() {
                            // Skip it if it's already in our chain
                            if !skipped_nodes.contains(child) {
                                // Since compressed items are intended to be displayed as a single item, we add 1 to the depth.
                                let new_depth = depth + 1;
                                self.stack.push((child.clone(), new_depth));
                            }
                        }
                    }
                }

                // Return the compressed item
                return Some(compressed_item);
            }

            // Normal case - not a compressed directory chain
            if let FileTreeNode::Directory(_, children) = &current_node {
                if !children.is_empty() {
                    for child_node in children.iter().rev() {
                        self.stack.push((child_node.clone(), depth + 1));
                    }
                }
            }

            // Return the current node (not compressed)
            Some(FlattenedTreeItem {
                is_root: depth == 0,
                node: current_node,
                depth,
            })
        } else {
            None // Stack is empty, iteration finished.
        }
    }
}

/// Represents a collection of `FileTree`s.
///
/// This structure is useful when dealing with multiple independent directory structures
/// or a set of paths that don't share a single common root. For example, a list of
/// top-level directories like `/home`, `/etc`, and `/var` would each form a separate
/// `FileTree` within a `FileTreeCollection`.
///
/// Paths are inserted into the collection, and they are automatically routed to an
/// existing `FileTree` if their root component matches, or a new `FileTree` is
/// created if necessary.
#[derive(Clone, Debug)]
pub struct FileTreeCollection {
    file_trees: Vec<FileTree>,
    flattened_file_trees: Option<Arc<Vec<FlattenedTreeItem>>>,
}

impl FileTreeCollection {
    /// Creates a new, empty `FileTreeCollection`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use photobook_rs::file_tree::FileTreeCollection;
    /// let collection = FileTreeCollection::new();
    /// assert!(collection.trees().is_empty());
    /// ```
    pub fn new() -> Self {
        FileTreeCollection {
            file_trees: Vec::new(),
            flattened_file_trees: None,
        }
    }

    /// Lazily evaluated flattened file tree
    pub fn flattened_file_trees(&mut self) -> Arc<Vec<FlattenedTreeItem>> {
        match &self.flattened_file_trees {
            Some(flattened_file_trees) => flattened_file_trees.clone(),
            None => {
                let flattened_file_trees = self.iter().collect();
                self.flattened_file_trees = Some(Arc::new(flattened_file_trees));
                self.flattened_file_trees.as_ref().unwrap().clone()
            }
        }
    }

    /// Inserts a path into the appropriate `FileTree` within the collection.
    ///
    /// The method first determines the root component of `path_to_insert` (e.g., "dir1" for
    /// "dir1/subdir/file.txt", or an empty path for `Path::new("")`).
    ///
    /// * If a `FileTree` already exists in the collection whose root path matches this
    ///   root component, `path_to_insert` is inserted into that existing `FileTree`.
    /// * Otherwise, a new `FileTree` is created, rooted at this root component. The full
    ///   `path_to_insert` is then inserted into this new tree to build out its structure.
    ///   The new tree is then added to the collection.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::path::Path;
    /// # use photobook_rs::file_tree::FileTreeCollection;
    /// let mut collection = FileTreeCollection::new();
    ///
    /// // Inserts "project_a/src/main.rs". Creates a new FileTree for "project_a".
    /// collection.insert(Path::new("project_a/src/main.rs"));
    ///
    /// // Inserts "project_a/README.md". Adds to the existing "project_a" FileTree.
    /// collection.insert(Path::new("project_a/README.md"));
    ///
    /// // Inserts "project_b/data/config.json". Creates a new FileTree for "project_b".
    /// collection.insert(Path::new("project_b/data/config.json"));
    ///
    /// assert_eq!(collection.trees().len(), 2);
    /// ```
    pub fn insert(&mut self, path_to_insert: &Path) {
        // Clear the flattened trees so they can be recomputed
        self.flattened_file_trees = None;
        // Determine the root component of the path to decide which tree it belongs to.
        // Path::new("") results in an empty PathBuf, suitable for "empty" root.
        let root_component_of_path_to_insert = path_to_insert
            .components()
            .next()
            .map_or_else(PathBuf::new, |comp| PathBuf::from(comp.as_os_str()));

        // Try to find an existing tree that is rooted at this component.
        if let Some(existing_tree) = self
            .file_trees
            .iter_mut()
            .find(|tree| tree.root.path() == &root_component_of_path_to_insert)
        {
            existing_tree.insert(path_to_insert);
        } else {
            // No existing tree found for this root component. Create a new one.
            // The new tree will be rooted at `root_component_of_path_to_insert`.
            let mut new_tree = FileTree::new(&root_component_of_path_to_insert);
            // Then, the full `path_to_insert` is inserted into this new tree
            // to build out its structure relative to its own root.
            new_tree.insert(path_to_insert);
            self.file_trees.push(new_tree);
        }
    }

    /// Gets a reference to the slice of `FileTree`s in this collection.
    ///
    /// This allows inspection of the individual trees managed by the collection.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::path::Path;
    /// # use photobook_rs::file_tree::FileTreeCollection;
    /// let mut collection = FileTreeCollection::new();
    /// collection.insert(Path::new("dir_a/file1.txt"));
    /// collection.insert(Path::new("dir_b/file2.txt"));
    ///
    /// let trees = collection.trees();
    /// assert_eq!(trees.len(), 2);
    /// // You can now inspect trees[0], trees[1], etc.
    /// ```
    pub fn trees(&self) -> &[FileTree] {
        &self.file_trees
    }

    /// Create a standard iterator over the collection
    pub fn iter(&self) -> FileTreeIterator {
        let mut stack = Vec::new();
        let file_trees = self.file_trees.clone();

        for tree in file_trees.iter().rev() {
            stack.push((tree.root.clone(), 0));
        }

        FileTreeIterator::new(stack)
    }

    /// Removes a path from the appropriate `FileTree` within the collection.
    ///
    /// The method first determines the root component of `path_to_remove` to find
    /// the correct tree, then removes the path from that tree. If removing the path
    /// results in an empty tree (only containing the root), the entire tree is
    /// removed from the collection.
    ///
    /// # Returns
    /// * `true` if the path was found and removed
    /// * `false` if the path was not found
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::path::Path;
    /// # use photobook_rs::file_tree::FileTreeCollection;
    /// let mut collection = FileTreeCollection::new();
    /// collection.insert(Path::new("project_a/src/main.rs"));
    /// collection.insert(Path::new("project_a/README.md"));
    ///
    /// // Remove a file
    /// let removed = collection.remove(Path::new("project_a/src/main.rs"));
    /// assert!(removed);
    /// ```
    pub fn remove(&mut self, path_to_remove: &Path) -> bool {
        // Clear the flattened trees so they can be recomputed
        self.flattened_file_trees = None;

        // Determine the root component of the path to find which tree it belongs to
        let root_component_of_path_to_remove = path_to_remove
            .components()
            .next()
            .map_or_else(PathBuf::new, |comp| PathBuf::from(comp.as_os_str()));

        // Find the tree that contains this path
        for i in 0..self.file_trees.len() {
            if self.file_trees[i].root.path() == &root_component_of_path_to_remove {
                let removed = self.file_trees[i].remove(path_to_remove);

                if removed {
                    // Check if the tree is now empty (only contains the root with no children)
                    match &self.file_trees[i].root {
                        FileTreeNode::Directory(_, children) if children.is_empty() => {
                            // If the directory is empty and we're removing the root itself
                            if self.file_trees[i].root.path() == path_to_remove {
                                self.file_trees.remove(i);
                            }
                        }
                        FileTreeNode::File(_) => {
                            // If it's a file and we're removing the root file itself
                            if self.file_trees[i].root.path() == path_to_remove {
                                self.file_trees.remove(i);
                            }
                        }
                        _ => {}
                    }
                    return true;
                }
                break;
            }
        }

        false
    }
}

/// Implements `Display` for `FileTreeCollection` to print all trees it contains.
///
/// Each `FileTree` in the collection is printed sequentially, separated by "---".
/// A header "Root Tree: [path]" precedes each tree's display.
/// If the collection is empty, it prints "(Empty File Collection)".
impl Display for FileTreeCollection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.file_trees.is_empty() {
            return writeln!(f, "(Empty File Collection)");
        }
        for (i, tree) in self.file_trees.iter().enumerate() {
            if i > 0 {
                writeln!(f, "\n---")?; // Separator for multiple trees in the collection
            }
            // Each FileTree's Display implementation handles its own formatting.
            // A header is added here to indicate the root of the current tree being printed.
            writeln!(f, "Root Tree: {}", tree.root.path().to_string_lossy())?;
            write!(f, "{}", tree)?;
        }
        fmt::Result::Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_file_tree_remove_file() {
        let mut tree = FileTree::new(Path::new("project"));
        tree.insert(Path::new("project/src/main.rs"));
        tree.insert(Path::new("project/README.md"));

        // Remove a file
        let removed = tree.remove(Path::new("project/src/main.rs"));
        assert!(removed);

        // Try to remove non-existent file
        let not_removed = tree.remove(Path::new("project/src/lib.rs"));
        assert!(!not_removed);
    }

    #[test]
    fn test_file_tree_remove_directory() {
        let mut tree = FileTree::new(Path::new("project"));
        tree.insert(Path::new("project/src/main.rs"));
        tree.insert(Path::new("project/src/lib.rs"));

        // Remove the entire src directory
        let removed = tree.remove(Path::new("project/src"));
        assert!(removed);
    }

    #[test]
    fn test_file_tree_collection_remove() {
        let mut collection = FileTreeCollection::new();
        collection.insert(Path::new("project_a/src/main.rs"));
        collection.insert(Path::new("project_a/README.md"));
        collection.insert(Path::new("project_b/config.json"));

        // Remove a file from project_a
        let removed = collection.remove(Path::new("project_a/src/main.rs"));
        assert!(removed);

        // Try to remove non-existent file
        let not_removed = collection.remove(Path::new("project_c/test.txt"));
        assert!(!not_removed);

        // Remove the entire project_b
        let removed = collection.remove(Path::new("project_b"));
        assert!(removed);
    }

    #[test]
    fn test_file_tree_collection_remove_empty_tree() {
        let mut collection = FileTreeCollection::new();
        collection.insert(Path::new("project/file.txt"));

        // Remove the only file, which should remove the entire tree
        let removed = collection.remove(Path::new("project/file.txt"));
        assert!(removed);

        // Collection should now be empty
        assert!(collection.trees().is_empty());
    }
}
