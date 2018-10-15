use std::{
    io::{self, Write},
    slice::Iter,
};

use indexmap::IndexSet;
use memchr::Memchr;

#[derive(Clone, Copy)]
enum OpName {
    Union,
    Intersect,
}

type TextVec = Vec<u8>;
type TextSlice = [u8];

trait SetExpression
where
    Self: Sized,
    // We can't say Sized + IntoLineIterator: rustc complains that there's
    // no implementation for type Foo, just for type &'a Foo
{
    fn operate(&mut self, text: &TextSlice);
    fn finish(&mut self) {}
    fn iter<'me>(&'me self) -> Box<dyn Iterator<Item = &'me TextSlice> + 'me>;
}

// I can't figure out how to implement this function inside the `SetExpression` trait,
// so every `impl trait SetExpression` will have have a `write_to` function that
// just calls `rite_to`
//
fn rite_to(zelf: &impl SetExpression, out: &mut impl Write) {
    for line in zelf.iter() {
        out.write_all(line.as_ref()).unwrap();
    }
}

type UnionSet = IndexSet<TextVec>;

trait UnionSetExt {
    fn init(text: TextVec) -> Self;
}

impl UnionSetExt for UnionSet {
    // The first operand is initialized by calling the `LineSet`'s initialization method.
    fn init(text: TextVec) -> Self {
        UnionSet::init_from_slice(&text)
    }
}

impl SetExpression for UnionSet {
    // For subsequent operands we simply insert each line into the hash
    fn operate(&mut self, text: &TextSlice) {
        self.insert_all_lines(&text);
    }
    fn iter<'me>(&'me self) -> Box<dyn Iterator<Item = &'me TextSlice> + 'me> {
        Box::new(self.iter().map(|v| v.as_slice()))
    }
}

// For an `IntersectSet` all result lines will be from the
// first file operand, so we can avoid additional allocations by keeping its
// text in memory and using subslices of its text as the members of the set.
type IntersectSet<'data> = IndexSet<&'data TextSlice>;

trait IntersectSetExt<'data> {
    fn init(text: &'data TextVec) -> Self;
}

impl<'data> IntersectSetExt<'data> for IntersectSet<'data> {
    fn init(text: &'data TextVec) -> Self {
        SliceSet::init_from_slice(text)
    }
}

// For subsequent operands, we take a `SliceSet` `s` of the operand's text and
// keep only those lines that occur in `s`.
impl<'data> SetExpression for IntersectSet<'data> {
    fn operate(&mut self, text: &TextSlice) {
        let other = SliceSet::init_from_slice(text);
        self.retain(|x| other.contains(x));
    }
    fn iter<'me>(&'me self) -> Box<dyn Iterator<Item = &'me TextSlice> + 'me> {
        // Set<&VecSlice>
        // .iter => &&VecSlice
        // .cloned => &VecSlice
        Box::new(self.iter().cloned())
    }
}

fn do_calculation(op: OpName, mut texts: Iter<TextVec>) {
    let txt = texts.next().unwrap();
    match op {
        OpName::Union => calculate_and_print(&mut UnionSet::init(txt.to_vec()), texts),
        OpName::Intersect => calculate_and_print(&mut IntersectSet::init(&txt), texts),
    }
}

fn calculate_and_print<T>(set: &mut T, texts: Iter<TextVec>)
where T: SetExpression
{
    for txt in texts {
        set.operate(txt);
    }
    set.finish();
    let stdout_for_locking = io::stdout();
    let mut stdout = stdout_for_locking.lock();
    rite_to(set, &mut stdout);
}

// Sets are implemented as variations on the `IndexSet` type
//
trait LineSet<'a>
where
    Self: Default,
{
    // The only method that implementations need to define is `insert_line`
    fn insert_line(&mut self, line: &'a TextSlice);

    // The `insert_all_lines` method breaks `text` down into lines and inserts
    // each of them into `self`
    fn insert_all_lines(&mut self, text: &'a TextSlice) {
        let mut begin = 0;
        for end in Memchr::new(b'\n', text) {
            self.insert_line(&text[begin..=end]);
            begin = end + 1;
        }
        if begin < text.len() {
            self.insert_line(&text[begin..]);
        }
    }
    // We initialize a `LineSet` from `text` by inserting every line contained
    // in text into an empty hash.
    fn init_from_slice(text: &'a TextSlice) -> Self {
        let mut set = Self::default();
        set.insert_all_lines(text);
        set
    }
}

// The simplest `LineSet` is a `SliceSet`, whose members (hash keys) are slices
// borrowed from a text string, each slice corresponding to a line.
//
type SliceSet<'a> = IndexSet<&'a TextSlice>;
impl<'a> LineSet<'a> for SliceSet<'a> {
    fn insert_line(&mut self, line: &'a TextSlice) {
        self.insert(line);
    }
}

// The next simplest set is a `UnionSet`, which we use to calculate the union
// of the lines which occur in at least one of a sequence of files. Rather than
// keep the text of all files in memory, we allocate a `TextVec` for each set member.
//
impl<'a> LineSet<'a> for UnionSet {
    fn insert_line(&mut self, line: &'a TextSlice) {
        self.insert(line.to_vec());
    }
}

fn main() {
    let txt_a = b"now is the time
now is the hour
there is the rhyme
but where is the flower?
".to_vec();
    let txt_b = b"but where is the flower?
eh? what's that you say?
now is the hour
there is the rhyme
and there's a bunny on road
and there's a bunny on road
".to_vec();
    let texts = vec![txt_a, txt_b];

    println!("\nUnion =========================");
    do_calculation(OpName::Union, texts.iter());

    println!("\nIntersection =========================");
    do_calculation(OpName::Intersect, texts.iter());
}
