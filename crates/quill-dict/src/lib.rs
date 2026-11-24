// The .DIC format and its builder: a set of words as a minimal deterministic automaton (a DAWG) in 4-byte edges.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// File layout (little-endian):
//
// | Offset | Size | |
// |---|---|---|
// | 0 | 4 | `QDIC` |
// | 4 | 2 | version (1) |
// | 6 | 2 | language code (1 en_CA, 2 en_US, 3 en_GB) |
// | 8 | 4 | edge count |
// | 12 | 4 | index of the root node's first edge (always 0) |
// | 16 | 4 | word count |
// | 20 | 4 | offset of the licence text |
// | 24 | 4 | length of the licence text |
// | 28 | 4 | reserved |
// | 32 | 4 each | the edges |
//
// An edge is a `u32`: bits 0 to 7 the letter (Windows-1252), bit 8 set when a word ends with this letter, bit 9 set on the last edge
// of its node, bits 10 to 31 the index of the first edge of the node it leads to (0 for none; the root is never a child). A node is a
// run of edges in letter order ending with one that has bit 9. Nodes are laid out breadth first, so the edges near the root, which
// every lookup uses, share the first pages. The licence text follows the edges.
use std::collections::HashMap;

/// The magic at the start of a `.DIC`.
pub const MAGIC: &[u8; 4] = b"QDIC";
/// Bytes before the first edge.
pub const HEADER: usize = 32;
/// Set on an edge where a word ends.
pub const END: u32 = 1 << 8;
/// Set on the last edge of a node.
pub const LAST: u32 = 1 << 9;
/// The child index starts at this bit.
pub const CHILD_SHIFT: u32 = 10;

struct Trie {
	/// Per node: whether a word ends here, and the children as (letter, node).
	end: Vec<bool>,
	kids: Vec<Vec<(u8, u32)>>,
}

impl Trie {
	fn new() -> Self {
		Trie {
			end: vec![false],
			kids: vec![Vec::new()],
		}
	}

	fn add(&mut self, word: &[u8]) {
		let mut at = 0usize;
		for &b in word {
			let next = match self.kids[at].iter().find(|(l, _)| *l == b) {
				Some((_, n)) => *n as usize,
				None => {
					self.end.push(false);
					self.kids.push(Vec::new());
					let n = self.end.len() - 1;
					self.kids[at].push((b, n as u32));
					n
				}
			};
			at = next;
		}
		self.end[at] = true;
	}
}

/// A node after minimisation: its edges (letter, ends-a-word, child), where a node that is a suffix of another is the same node.
type Sig = Vec<(u8, bool, u32)>;

/// Builds the bytes of a `.DIC` from words (any order, duplicates allowed, no empty words), a language code and the licence text.
pub fn build(words: &[Vec<u8>], language: u16, licence: &str) -> Vec<u8> {
	let mut sorted: Vec<&Vec<u8>> = words.iter().filter(|w| !w.is_empty()).collect();
	sorted.sort();
	sorted.dedup();
	let mut trie = Trie::new();
	for w in &sorted {
		trie.add(w);
	}
	// Minimise from the leaves up: two nodes are one when their edges (letter, end flag, canonical child) are equal.
	let n = trie.end.len();
	let mut canon = vec![u32::MAX; n];
	let mut unique: HashMap<Sig, u32> = HashMap::new();
	let mut nodes: Vec<Sig> = Vec::new();
	// Children have larger indexes than their parents (a trie is built top down), so a reverse pass is a post-order.
	for i in (0..n).rev() {
		let mut sig: Sig = trie.kids[i]
			.iter()
			.map(|&(l, c)| (l, trie.end[c as usize], canon[c as usize]))
			.collect();
		sig.sort_by_key(|e| e.0);
		// A child with no edges of its own is the empty node, which edges point to as "no child" (0 is the root, never a child).
		let id = *unique.entry(sig.clone()).or_insert_with(|| {
			nodes.push(sig);
			(nodes.len() - 1) as u32
		});
		canon[i] = id;
	}
	let root = canon[0];
	// The words that end on an edge are flagged on the edge, so a leaf (no edges) is represented as "no child": map it to u32::MAX.
	let empty = nodes.iter().position(|s| s.is_empty()).map(|p| p as u32);
	// Lay the nodes out breadth first from the root, assigning each reachable non-empty node its first edge index.
	let mut order: Vec<u32> = vec![root];
	let mut first: HashMap<u32, u32> = HashMap::new();
	let mut edges = 0u32;
	let mut i = 0;
	while i < order.len() {
		let id = order[i];
		first.insert(id, edges);
		edges += nodes[id as usize].len() as u32;
		for &(_, _, c) in &nodes[id as usize] {
			if Some(c) != empty && !first.contains_key(&c) && !order.contains(&c) {
				order.push(c);
			}
		}
		i += 1;
	}
	let mut out = Vec::with_capacity(HEADER + edges as usize * 4 + licence.len());
	out.extend_from_slice(MAGIC);
	out.extend_from_slice(&1u16.to_le_bytes());
	out.extend_from_slice(&language.to_le_bytes());
	out.extend_from_slice(&edges.to_le_bytes());
	out.extend_from_slice(&0u32.to_le_bytes());
	out.extend_from_slice(&(sorted.len() as u32).to_le_bytes());
	let lic_off = (HEADER as u32) + edges * 4;
	out.extend_from_slice(&lic_off.to_le_bytes());
	out.extend_from_slice(&(licence.len() as u32).to_le_bytes());
	out.extend_from_slice(&0u32.to_le_bytes());
	for &id in &order {
		let sig = &nodes[id as usize];
		for (k, &(letter, ends, child)) in sig.iter().enumerate() {
			let mut e = u32::from(letter);
			if ends {
				e |= END;
			}
			if k + 1 == sig.len() {
				e |= LAST;
			}
			if Some(child) != empty {
				e |= first[&child] << CHILD_SHIFT;
			}
			out.extend_from_slice(&e.to_le_bytes());
		}
	}
	out.extend_from_slice(licence.as_bytes());
	out
}

/// Reads a SCOWL list (ISO 8859-1, one word a line, CRLF or LF) into byte words, which Windows-1252 shares for these characters.
pub fn read_list(text: &[u8]) -> Vec<Vec<u8>> {
	text.split(|b| *b == b'\n')
		.map(|l| l.strip_suffix(b"\r").unwrap_or(l))
		.filter(|l| !l.is_empty())
		.map(<[u8]>::to_vec)
		.collect()
}
