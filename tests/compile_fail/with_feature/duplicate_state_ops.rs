use scnr2::scanner;

scanner! {
    BadScanner {
        state n: count(0..=4);
        mode INITIAL {
            token capture("#", n) + validate("#", n) => 1;
        }
    }
}

fn main() {}
