use scnr2::scanner;

scanner! {
    BadScanner {
        state n: count(0..=4);
        mode INITIAL {
            token validate("#", n, ..n) => 1;
        }
    }
}

fn main() {}
