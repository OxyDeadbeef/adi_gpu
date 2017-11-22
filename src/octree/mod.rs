/// octree.rs    A simple octree for spatial searching.
///
/// Copyright (c) 2017  Douglas P Lau
pub mod geom;

use std::cmp::Ordering;
use std::fmt;

use self::geom::Vec3;
use self::geom::BBox;
use self::geom::Pos;

/// An octree is a DAG that can quickly search for points in 3D space.
///
/// The bounding box of the root node contains all points in the octree.
/// If a point outside the bounding box is added, a new root node is created
/// which contains the old root as one of its octants.  This process is repeated
/// until the point is contained.
///
/// The nodes are stored in a vector, and are indexed using a 32-bit node ID.
/// This saves memory over using pointers on 64-bit systems.  Node ID 1 is the
/// first node in the vector.
///
pub struct Octree {
	nodes: Vec<Node>,
	garbage: Vec<u32>,
	sorted: Vec<u32>,
	bbox: BBox,
	root: u32,
	n_points: u32,
}

const LINK: u32 = 7;		// link to coincident leaf nodes
const LEAF: u32 = !0;	// max u32 value (invalid handle)

/// A node can be either a branch or a leaf.
///
/// A branch can have up to 8 child nodes (each octant adjacent to the center).
///
/// A leaf can store up to 6 points; the first child must contain a LEAF
/// sentinel value, and the last may link to another leaf node with only
/// coincident points.
///
/// Each node has an implicit bounding box determined by its position in the
/// octree.  The bounding box contains all descendant nodes.
///
struct Node {
	child: [u32; 8],	// child node handles
}

impl fmt::Debug for Node {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		if self.is_leaf() {
			try!(write!(f, "leaf: {:?}", self.leaf_children()));
			let l = self.link();
			if l > 0 {
				try!(write!(f, " link: {:?}", l));
			}
			Ok(())
		} else {
			write!(f, "branch: {:?}", self.child)
		}
	}
}

impl Node {
	/// Create a new leaf node
	fn new_leaf() -> Node {
		Node {
			child: [LEAF, 0, 0, 0, 0, 0, 0, 0],
		}
	}

	/// Create a new branch node
	fn new_branch() -> Node {
		Node { child: [0; 8], }
	}

	/// Test if a node is empty
	fn is_empty(&self) -> bool {
		self.child == [LEAF, 0, 0, 0, 0, 0, 0, 0] ||
		self.child == [0; 8]
	}

	/// Test if a node is a leaf
	fn is_leaf(&self) -> bool {
		self.child[0] == LEAF
	}

	/// Test if a node is a branch
	fn is_branch(&self) -> bool {
		!self.is_leaf()
	}

	/// Get link to next link node ID
	fn link(&self) -> u32 {
		assert!(self.is_leaf());
		self.child[LINK as usize]
	}

	/// Find the first empty child slot
	fn empty_slot(&self) -> Option<usize> {
		self.child.iter().position(|v| *v == 0)
	}

	/// Find the first open child slot in a leaf
	fn open_slot(&self) -> Option<usize> {
		assert!(self.is_leaf());
		let slot = self.empty_slot();
		if let Some(s) = slot {
			if s < 7 {
				return slot;
			}
		}
		None
	}

	/// Check if a leaf node is full
	fn is_full(&self) -> bool {
		assert!(self.is_leaf());
		match self.open_slot() {
			Some(_) => false,
			None => true,
		}
	}

	/// Check if all points are coincident with the given point
	fn all_coincident(&self, pts: &Pos, p: Vec3) -> bool {
		assert!(self.is_leaf());
		assert!(self.is_full());
		p == pts.pos(self.child[1]) &&
		p == pts.pos(self.child[2]) &&
		p == pts.pos(self.child[3]) &&
		p == pts.pos(self.child[4]) &&
		p == pts.pos(self.child[5]) &&
		p == pts.pos(self.child[6])
	}

	/// Add a point to a leaf node
	fn add_leaf(&mut self, hnd: u32) {
		assert!(self.is_leaf());
		assert!(!self.is_full());
		let s = self.open_slot().unwrap();
		self.child[s as usize] = hnd;
	}

	/// Remove a point from a leaf node
	fn remove_leaf(&mut self, hnd: u32) -> bool {
		assert!(self.is_leaf());
		if let Some(s) = self.child.iter().position(|v| *v == hnd) {
			self.child[s] = 0;
			true
		} else {
			false
		}
	}

	// TODO: Don't be forced to allocate a vector
	/// Get a vector containing the leaf children
	fn leaf_children(&self) -> Vec<u32> {
		assert!(self.is_leaf());
		let mut children = Vec::new();
		for ch in 1 .. 7 {
			let hnd = self.child[ch];
			if hnd > 0 {
				children.push(hnd);
			}
		}
		children
	}

	/// Determine which child for a branch point
	fn which_child(c: Vec3, p: Vec3) -> u32 {
		match (p.x < c.x, p.y < c.y, p.z < c.z) {
			(true,  true,  true)  => 0,
			(true,  true,  false) => 1,
			(true,  false, true)  => 2,
			(true,  false, false) => 3,
			(false, true,  true)  => 4,
			(false, true,  false) => 5,
			(false, false, true)  => 6,
			(false, false, false) => 7,
		}
	}

	/// Calculate the center of a child node
	fn child_center(ch: u32, c: Vec3, h: f32) -> Vec3 {
		match ch {
			0 => Vec3::new(c.x - h, c.y - h, c.z - h),
			1 => Vec3::new(c.x - h, c.y - h, c.z + h),
			2 => Vec3::new(c.x - h, c.y + h, c.z - h),
			3 => Vec3::new(c.x - h, c.y + h, c.z + h),
			4 => Vec3::new(c.x + h, c.y - h, c.z - h),
			5 => Vec3::new(c.x + h, c.y - h, c.z + h),
			6 => Vec3::new(c.x + h, c.y + h, c.z - h),
			_ => Vec3::new(c.x + h, c.y + h, c.z + h),
		}
	}

	/// Calculate the bounding box of a child node
	fn child_bbox(ch: u32, bbox: BBox) -> BBox {
		assert!(bbox.half_len > 0.0);
		let half_len = bbox.half_len / 2.0;
		let center = Node::child_center(ch, bbox.center, half_len);
		BBox { center: center, half_len: half_len }
	}
}

impl Octree {
	/// Create a new octree
	pub fn new() -> Octree {
		Octree {
			nodes: Vec::new(),
			garbage: Vec::new(),
			sorted: Vec::new(),
			bbox: BBox::empty(),
			root: 0,
			n_points: 0,
		}
	}

	/// Add a new node
	fn new_node(&mut self, n: Node) -> u32 {
		if let Some(i) = self.garbage.pop() {
			let k = i;
			self.nodes[(k - 1) as usize] = n;
			k
		} else {
			self.nodes.push(n);
			self.nodes.len() as u32
		}
	}

	/// Add a new leaf node
	fn new_leaf(&mut self) -> u32 {
		self.new_node(Node::new_leaf())
	}

	/// Add a new branch node
	fn new_branch(&mut self) -> u32 {
		self.new_node(Node::new_branch())
	}

	/// Add a point in the octree
	pub fn add(&mut self, hnd: u32, pts: &Pos) {
		match self.n_points {
			0 => self.add_0(hnd, pts),
			_ => self.add_n(hnd, pts),
		}
	}

	/// Add a point when empty
	fn add_0(&mut self, hnd: u32, pts: &Pos) {
		assert!(self.n_points == 0);
		let p = pts.pos(hnd);
		self.nodes.clear();
		self.garbage.clear();
		let i = self.new_leaf();
		self.nodes[(i - 1) as usize].add_leaf(hnd);
		self.bbox = BBox::new(p);
		self.root = 1;
		self.n_points = 1;
	}

	/// Add a point when not empty
	fn add_n(&mut self, hnd: u32, pts: &Pos) {
		assert!(self.n_points > 0);
		let p = pts.pos(hnd);
		while !self.bbox.contains(p) {
			self.grow_root(p);
		}
		self.add_inside(hnd, pts);
	}

	/// Grow the root node
	fn grow_root(&mut self, p: Vec3) {
		assert!(!self.bbox.contains(p));
		let center = self.bbox.center;
		let i = self.root - 1;
		self.bbox.extend(p);
		if self.nodes[i as usize].is_branch() {
			let ch = Node::which_child(self.bbox.center, center);
			let k = self.new_branch();
			self.nodes[(k - 1) as usize].child[ch as usize] = self.root;
			self.root = k;
		}
	}

	/// Add a point within the bounds
	fn add_inside(&mut self, hnd: u32, pts: &Pos) {
		let p = pts.pos(hnd);
		assert!(self.bbox.contains(p));
		let (mut i, mut bbox) = self.find_leaf_grow(p);
		while self.nodes[i as usize].is_full() {
			let (j, bb) = self.grow_leaf(i, bbox, pts, p);
			i = j;
			bbox = bb;
		}
		self.nodes[i as usize].add_leaf(hnd);
		self.n_points += 1;
	}

	/// Find the leaf node for a point (grow it if necessary)
	fn find_leaf_grow(&mut self, p: Vec3) -> (u32, BBox) {
		assert!(self.bbox.contains(p));
		let mut i = self.root - 1;
		let mut bbox = self.bbox;
		while self.nodes[i as usize].is_branch() {
			let (j, bb) = self.follow_branch_grow(i, bbox, p);
			i = j;
			bbox = bb;
		}
		(i, bbox)
	}

	/// Follow a branch or grow a leaf node
	fn follow_branch_grow(&mut self, i: u32, bbox: BBox, p: Vec3) ->
		(u32, BBox)
	{
		assert!(self.nodes[i as usize].is_branch());
		let ch = Node::which_child(bbox.center, p);
		let j = self.nodes[i as usize].child[ch as usize];
		let bb = Node::child_bbox(ch, bbox);
		if j > 0 {
			(j - 1, bb)
		} else {
			let k = self.new_leaf();
			self.nodes[i as usize].child[ch as usize] = k;
			(k - 1, bb)
		}
	}

	/// Grow a leaf node into a branch or link
	fn grow_leaf(&mut self, i: u32, bbox: BBox, pts: &Pos, p: Vec3) ->
		(u32, BBox)
	{
		assert!(self.nodes[i as usize].is_leaf());
		assert!(self.nodes[i as usize].is_full());
		if self.nodes[i as usize].all_coincident(pts, p) {
			self.grow_leaf_link(i, bbox)
		} else {
			self.grow_leaf_branch(i, bbox.center, pts);
			self.follow_branch_grow(i, bbox, p)
		}
	}

	/// Grow a leaf node linking to another leaf
	fn grow_leaf_link(&mut self, i: u32, bbox: BBox) -> (u32, BBox) {
		assert!(self.nodes[i as usize].is_leaf());
		assert!(self.nodes[i as usize].is_full());
		let j = self.nodes[i as usize].link();
		if j > 0 {
			(j - 1, bbox)
		} else {
			let k = self.new_leaf();
			// Link to new coincident leaf
			self.nodes[i as usize].child[LINK as usize] = k;
			(k - 1, bbox)
		}
	}

	/// Grow a full leaf into a branch
	fn grow_leaf_branch(&mut self, i: u32, center: Vec3, pts: &Pos) {
		assert!(self.nodes[i as usize].is_leaf());
		assert!(self.nodes[i as usize].is_full());
		let mut br = Node::new_branch();
		let link = self.nodes[i as usize].link();
		for hnd in self.nodes[i as usize].leaf_children() {
			let p = pts.pos(hnd);
			let ch = Node::which_child(center, p);
			let j = br.child[ch as usize] as usize;
			if j > 0 {
				// NOTE: if there is a link, all children
				//       must be coincident
				assert!(self.nodes[j - 1].link() == link);
				self.nodes[j - 1].add_leaf(hnd);
			} else {
				let k = self.new_leaf();
				// Preserve link to coincident leaves
				self.nodes[(k - 1) as usize].child[LINK as usize] = link;
				self.nodes[(k - 1) as usize].add_leaf(hnd);
				br.child[ch as usize] = k;
			}
		}
		self.nodes[i as usize] = br;
	}

	/// Remove a point from the octree
	pub fn remove(&mut self, hnd: u32, pts: &Pos) {
		if self.n_points > 0 {
			assert!(self.root > 0);
			let i = self.root - 1;
			let bbox = self.bbox;
			let p = pts.pos(hnd);
			self.remove_point(i, bbox, hnd, p);
		}
	}

	/// Remove a point within a bounding box
	fn remove_point(&mut self, i: u32, bbox: BBox, hnd: u32, p: Vec3) {
		if self.nodes[i as usize].is_branch() {
			self.remove_branch(i, bbox, hnd, p);
		} else {
			self.remove_leaf(i, hnd);
		}
	}

	/// Remove a point from a branch
	fn remove_branch(&mut self, i: u32, bbox: BBox, hnd: u32, p: Vec3) {
		assert!(self.nodes[i as usize].is_branch());
		let ch = Node::which_child(bbox.center, p);
		let j = self.nodes[i as usize].child[ch as usize];
		if j > 0 {
			let k = j - 1;
			let bb = Node::child_bbox(ch, bbox);
			self.remove_point(k, bb, hnd, p);
			if self.nodes[k as usize].is_empty() {
				self.nodes[i as usize].child[ch as usize] = 0;
				self.garbage.push(j);
			}
		}
	}

	/// Remove a point from a leaf
	fn remove_leaf(&mut self, i: u32, hnd: u32) {
		assert!(self.nodes[i as usize].is_leaf());
		if self.nodes[i as usize].remove_leaf(hnd) {
			self.n_points -= 1;
		}
		// FIXME: check for linked leaves
	}

	/// Return all the parents in order, starting from hnd's parent, ending
	/// at root.
	fn get_parents(&self, hnd: u32, pts: &Pos) -> Vec<u32> {
		let mut parents = Vec::new();
		let mut ch = self.root; // First push root node

		while ch != hnd {
			parents.push(ch);
			// Which of the 8 child octants is hnd in?
			ch = Node::which_child(self.bbox.center, pts.pos(hnd));
		}
		parents.reverse();
		parents
	}

	/// Children of i except hnd are sorted
	fn add_sorted_points_except(&mut self, hnd: u32, pts: &Pos, i: u32) {
		if self.nodes[i as usize].is_leaf() {
			for j in 1..6+1 {
				let ch = self.nodes[i as usize].child[j];

				// Only if there is a child and it is not hnd
				if ch != hnd && ch != 0 {
					self.sorted.push(ch);
				}
			}
		} else { // Branch
			for j in 1..6+1 {
				let ch = self.nodes[i as usize].child[j];

				if ch != 0 {
					self.add_sorted_points_except(hnd, pts,
						ch);
				}
			}
		}
	}

	/// Children of i except hnd are sorted
	fn add_all_sorted_points(&mut self, pts: &Pos, i: u32) {
		if self.nodes[i as usize].is_leaf() {
			for j in 1..6+1 {
				let ch = self.nodes[i as usize].child[j];

				if ch != 0 {
					self.sorted.push(ch);
				}
			}
		} else { // Branch
			for j in 1..6+1 {
				let ch = self.nodes[i as usize].child[j];

				if ch != 0 {
					self.add_all_sorted_points(pts, ch);
				}
			}
		}
	}

	/// Find node children
	fn find_node_ch(&mut self, pts: &Pos, i: u32) {
		if self.nodes[i as usize].is_leaf() {
			for hnd in self.nodes[i as usize].leaf_children() {
				self.sorted.push(hnd);
			}
			let j = self.nodes[i as usize].link();
			if j > 0 {
				self.find_node_ch(pts, j - 1);
			}
		} else {
			for ch in 0 .. 8 {
				let k = self.nodes[i as usize].child[ch as usize];
				if k > 0 {
					self.find_node_ch(pts, k - 1);
				}
			}
		}
	}

	/// Sort the octree nearest to farthest, while culling all outside of
	/// view frustum.
	pub fn nearest(&mut self, pts: &Pos, mat4: [f32; 16]) -> &Vec<u32> {
		self.sorted.clear();

		if self.root == 0 {
			return &self.sorted;
		}

		let hnd = self.root - 1;

		self.find_node_ch(pts, hnd);

		self.sorted.sort_unstable_by(|a, b| {
			let p = pts.pos(*a);
			let z1 = mat4[2] * p.x + mat4[6] * p.y + mat4[10] * p.z + mat4[14] * 1.0;
			let p = pts.pos(*b);
			let z2 = mat4[2] * p.x + mat4[6] * p.y + mat4[10] * p.z + mat4[14] * 1.0;

			if z1 > z2 {
				Ordering::Greater
			} else if z1 < z2 {
				Ordering::Less
			} else {
				Ordering::Equal
			}
		});

//		self.print(pts);

		&self.sorted
	}

	pub fn farthest(&mut self, pts: &Pos, mat4: [f32; 16]) -> &Vec<u32> {
		self.sorted.clear();

		if self.root == 0 {
			return &self.sorted;
		}

		let hnd = self.root - 1;

		self.find_node_ch(pts, hnd);

		self.sorted.sort_unstable_by(|a, b| {
			let p = pts.pos(*a);
			let z1 = mat4[2] * p.x + mat4[6] * p.y + mat4[10] * p.z + mat4[14] * 1.0;
			let p = pts.pos(*b);
			let z2 = mat4[2] * p.x + mat4[6] * p.y + mat4[10] * p.z + mat4[14] * 1.0;

			if z1 > z2 {
				Ordering::Less
			} else if z1 < z2 {
				Ordering::Greater
			} else {
				Ordering::Equal
			}
		});

		&self.sorted
	}

	/// Print the octree
	pub fn print(&self, pts: &Pos) {
		self.print_node(pts, self.root - 1, self.bbox, 0);
		println!("");
	}

	/// Print a node and its descendants
	fn print_node(&self, pts: &Pos, i: u32, bbox: BBox, t: u32) {
		let n = &self.nodes[i as usize];
		print!("\n{:3} ", i + 1);
		for _ in 0 .. t {
			print!("  ");
		}
		if n.is_leaf() {
			print!("leaf:");
			for hnd in n.leaf_children() {
				let p = pts.pos(hnd);
				print!(" {:?}_{:?}", hnd, p);
			}
			let j = n.link();
			if j > 0 {
				self.print_node(pts, j - 1, bbox, t);
			}
		} else {
			print!("{:?}", n);
			print!("\t{:?}", bbox);
			for ch in 0 .. 8 {
				let bb = Node::child_bbox(ch, bbox);
				let k = n.child[ch as usize];
				if k > 0 {
					self.print_node(pts, k - 1, bb, t + 1);
				}
			}
		}
	}

	/// Get the number of points within the octree.
	pub fn len(&self) -> u32 {
		self.n_points
	}
}

impl fmt::Debug for Octree {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		try!(write!(f, "octree: bbox: {:?}", self.bbox));
		try!(write!(f, "\n\troot: {:?}", self.root));
		try!(write!(f, "\n\tn_points: {:?}", self.len()));
		try!(write!(f, "\n\tnodes: {:?}", self.nodes.len()));
		try!(write!(f, "\n\tgarbage: {:?}", self.garbage.len()));
		Ok(())
	}
}

pub struct Points {
	pts: Vec<Vec3>,
}

impl Points {
	pub fn new() -> Points {
		Points {
			pts: Vec::new(),
		}
	}

	pub fn add(&mut self, p: Vec3) {
		self.pts.push(p);
	}

	pub fn wrt(&mut self, hnd: u32, vertex: Vec3) {
		self.pts[(hnd - 1) as usize] = vertex;
	}

	pub fn len(&self) -> usize {
		self.pts.len()
	}
}

impl Pos for Points {
	fn pos(&self, hnd: u32) -> Vec3 {
		self.pts[hnd as usize - 1]
	}
}

#[test]
fn test_octree() {
	let mut pts = Points::new();
	for x in 0 .. 100 {
		for y in 0 .. 100 {
			for z in 0 .. 100 {
				pts.add(Vec3::new(x, y, z));
			}
		}
	}
	let mut o = Octree::new();
	for i in 1 .. pts.len() + 1 {
		o.add(i as u32, &pts);
	}
}

#[test]
fn test_coincident() {
	let mut pts = Points::new();
	for _ in 0 .. 10 {
		pts.add(Vec3::new(0, 0, 0));
	}
	pts.add(Vec3::new(1,1,1));
	let mut o = Octree::new();
	for i in 1 .. pts.len() + 1 {
		o.add(i as u32, &pts);
	}
}

#[test]
fn test_add_remove() {
	let mut pts = Points::new();
	for x in 0 .. 10 {
		for y in 0 .. 10 {
			for z in 0 .. 10 {
				pts.add(Vec3::new(x, y, z));
			}
		}
	}
	let mut o = Octree::new();
	for i in 1 .. pts.len() + 1 {
		o.add(i as u32, &pts);
	}
	for i in 1 .. pts.len() + 1 {
		o.remove(i as u32, &pts);
	}
}
