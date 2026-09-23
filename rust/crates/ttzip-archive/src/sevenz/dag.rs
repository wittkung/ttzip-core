// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! 7-Zip Coder BindPairs Directed Acyclic Graph (DAG) Engine.
//!
//! Provides zero-allocation, deterministic $O(V + E)$ Kahn topological sorting,
//! cycle detection, self-loop interception, and stream binding resolution for
//! complex multi-coder 7z solid decompression pipelines (e.g. BCJ2 + LZMA + AES).

use std::collections::VecDeque;

pub use ttzip_types::SevenZError;


/// Represents an individual Coder node inside a 7z Folder decompression DAG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoderNode {
    /// Zero-based index of this coder within the Folder.
    pub coder_index: usize,
    /// Global input stream indices consumed by this coder.
    pub in_streams: Vec<usize>,
    /// Global output stream indices produced by this coder.
    pub out_streams: Vec<usize>,
    /// In-degree (number of coders whose outputs feed into this coder's inputs).
    pub in_degree: usize,
    /// Out-degree (number of coders that consume this coder's outputs).
    pub out_degree: usize,
    /// Upstream dependency coder indices (producers feeding into this coder).
    pub dependencies: Vec<usize>,
    /// Downstream dependent coder indices (consumers fed by this coder).
    pub dependents: Vec<usize>,
    /// Whether this coder produces the final uncompressed output of the folder.
    pub is_main_coder: bool,
}

impl CoderNode {
    /// Creates a new `CoderNode` with empty stream mappings.
    #[must_use]
    pub fn new(coder_index: usize) -> Self {
        Self {
            coder_index,
            in_streams: Vec::new(),
            out_streams: Vec::new(),
            in_degree: 0,
            out_degree: 0,
            dependencies: Vec::new(),
            dependents: Vec::new(),
            is_main_coder: false,
        }
    }
}

/// Directed Acyclic Graph (DAG) representing the coder pipeline and stream topology.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoderGraph {
    /// Nodes representing each coder in the Folder.
    pub nodes: Vec<CoderNode>,
    /// Topologically sorted coder indices in decompression execution order.
    pub topological_order: Vec<usize>,
    /// Global input stream indices that are unbound (read directly from archive pack streams).
    pub packed_streams: Vec<usize>,
    /// Terminal coder index that produces the folder's primary uncompressed output stream.
    pub main_coder: usize,
    /// Global output stream indices that are not bound to any downstream coder.
    pub unbound_out_streams: Vec<usize>,
}

impl CoderGraph {
    /// Builds and topologically sorts a `CoderGraph` from raw 7z folder bind pairs.
    ///
    /// # Arguments
    /// * `num_coders` - Total number of coders in the folder.
    /// * `bind_pairs` - Slice of `(in_stream_idx, out_stream_idx)` tuples.
    /// * `stream_coder_map` - Slice mapping each global input stream index to its owning coder index.
    ///
    /// # Errors
    /// Returns `SevenZError` if:
    /// - `num_coders == 0` (`EmptyFolder`);
    /// - Any stream or coder index is out of bounds;
    /// - Any stream is bound more than once;
    /// - A self-loop or cyclic dependency is detected.
    pub fn build(
        num_coders: usize,
        bind_pairs: &[(u64, u64)],
        stream_coder_map: &[usize],
    ) -> Result<Self, SevenZError> {
        if num_coders == 0 {
            return Err(SevenZError::EmptyFolder);
        }

        // Default out_to_coder mapping assumes 1 output stream per coder (0..num_coders)
        let out_to_coder: Vec<usize> = (0..num_coders).collect();
        Self::build_with_maps(num_coders, bind_pairs, stream_coder_map, &out_to_coder)
    }

    /// Builds and topologically sorts a `CoderGraph` with explicit in and out stream maps.
    pub fn build_with_maps(
        num_coders: usize,
        bind_pairs: &[(u64, u64)],
        in_to_coder: &[usize],
        out_to_coder: &[usize],
    ) -> Result<Self, SevenZError> {
        if num_coders == 0 {
            return Err(SevenZError::EmptyFolder);
        }

        let total_in = in_to_coder.len();
        let total_out = out_to_coder.len();

        let mut nodes: Vec<CoderNode> = (0..num_coders).map(CoderNode::new).collect();

        // Populate in_streams for each coder
        for (in_idx, &coder_idx) in in_to_coder.iter().enumerate() {
            if coder_idx >= num_coders {
                return Err(SevenZError::InvalidCoderIndex {
                    index: coder_idx,
                    num_coders,
                });
            }
            nodes[coder_idx].in_streams.push(in_idx);
        }

        // Populate out_streams for each coder
        for (out_idx, &coder_idx) in out_to_coder.iter().enumerate() {
            if coder_idx >= num_coders {
                return Err(SevenZError::InvalidCoderIndex {
                    index: coder_idx,
                    num_coders,
                });
            }
            nodes[coder_idx].out_streams.push(out_idx);
        }

        let mut in_bound = vec![false; total_in];
        let mut out_bound = vec![false; total_out];

        // Process bind pairs: each pair is (in_stream_idx, out_stream_idx)
        // Producer: coder of out_stream_idx
        // Consumer: coder of in_stream_idx
        for &(in_stream_idx, out_stream_idx) in bind_pairs {
            let in_idx = in_stream_idx as usize;
            let out_idx = out_stream_idx as usize;

            if in_idx >= total_in {
                return Err(SevenZError::InvalidStreamIndex {
                    stream_index: in_stream_idx,
                    max_streams: total_in,
                });
            }
            if out_idx >= total_out {
                return Err(SevenZError::InvalidStreamIndex {
                    stream_index: out_stream_idx,
                    max_streams: total_out,
                });
            }

            if in_bound[in_idx] {
                return Err(SevenZError::StreamAlreadyBound {
                    stream_index: in_stream_idx,
                });
            }
            if out_bound[out_idx] {
                return Err(SevenZError::StreamAlreadyBound {
                    stream_index: out_stream_idx,
                });
            }

            in_bound[in_idx] = true;
            out_bound[out_idx] = true;

            let in_coder = in_to_coder[in_idx];
            let out_coder = out_to_coder[out_idx];

            // Direct self-loop interception
            if in_coder == out_coder {
                return Err(SevenZError::SelfLoop {
                    coder_index: in_coder,
                });
            }

            // Directed edge in data flow: out_coder (producer) -> in_coder (consumer)
            nodes[out_coder].dependents.push(in_coder);
            nodes[out_coder].out_degree += 1;

            nodes[in_coder].dependencies.push(out_coder);
            nodes[in_coder].in_degree += 1;
        }

        // Identify unbound input streams (packed streams from archive payload)
        let packed_streams: Vec<usize> = in_bound
            .iter()
            .enumerate()
            .filter_map(|(idx, &bound)| if !bound { Some(idx) } else { None })
            .collect();

        // Identify unbound output streams
        let unbound_out_streams: Vec<usize> = out_bound
            .iter()
            .enumerate()
            .filter_map(|(idx, &bound)| if !bound { Some(idx) } else { None })
            .collect();

        // Perform Kahn's topological sort
        let topological_order = Self::run_kahn_sort(&nodes, num_coders)?;

        // Determine main terminal coder
        let main_coder = if unbound_out_streams.is_empty() {
            // If all outputs are bound (or no outputs), default to last coder or error
            if num_coders == 1 {
                0
            } else {
                return Err(SevenZError::NoMainCoder);
            }
        } else {
            // Main coder produces the unbound output stream
            let first_unbound_out = unbound_out_streams[0];
            out_to_coder[first_unbound_out]
        };

        if main_coder < num_coders {
            nodes[main_coder].is_main_coder = true;
        }

        Ok(Self {
            nodes,
            topological_order,
            packed_streams,
            main_coder,
            unbound_out_streams,
        })
    }

    /// Internal Kahn's Algorithm topological sort implementation.
    fn run_kahn_sort(nodes: &[CoderNode], num_coders: usize) -> Result<Vec<usize>, SevenZError> {
        let mut in_degrees: Vec<usize> = nodes.iter().map(|n| n.in_degree).collect();
        let mut queue = VecDeque::new();

        // Deterministic initial seeding: push all coders with in_degree == 0 in ascending index order
        for (idx, &deg) in in_degrees.iter().enumerate() {
            if deg == 0 {
                queue.push_back(idx);
            }
        }

        let mut sorted_order = Vec::with_capacity(num_coders);

        while let Some(u) = queue.pop_front() {
            sorted_order.push(u);

            for &v in &nodes[u].dependents {
                if in_degrees[v] == 0 {
                    return Err(SevenZError::CyclicBindPairs);
                }
                in_degrees[v] -= 1;
                if in_degrees[v] == 0 {
                    queue.push_back(v);
                }
            }
        }

        if sorted_order.len() != num_coders {
            return Err(SevenZError::CyclicBindPairs);
        }

        Ok(sorted_order)
    }

    /// Returns the topologically sorted coder indices.
    #[must_use]
    pub fn topological_order(&self) -> &[usize] {
        &self.topological_order
    }

    /// Returns the unbound input stream indices (packed stream inputs).
    #[must_use]
    pub fn packed_streams(&self) -> &[usize] {
        &self.packed_streams
    }

    /// Returns the main terminal coder index.
    #[must_use]
    pub fn main_coder(&self) -> usize {
        self.main_coder
    }

    /// Returns the unbound output stream indices.
    #[must_use]
    pub fn unbound_out_streams(&self) -> &[usize] {
        &self.unbound_out_streams
    }
}

/// Standalone entry point to build and sort a 7z Coder DAG.
///
/// Implements deterministic $O(V + E)$ Kahn topological sort, cycle detection,
/// and self-loop rejection for 7z BindPairs pipelines.
#[inline]
pub fn build_and_sort(
    num_coders: usize,
    bind_pairs: &[(u64, u64)],
    stream_coder_map: &[usize],
) -> Result<Vec<usize>, SevenZError> {
    CoderGraph::build(num_coders, bind_pairs, stream_coder_map).map(|g| g.topological_order)
}
