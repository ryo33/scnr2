use scnr2::scanner;

scanner! {
    BadScanner {
        state n: count(0..=4);
        state n: count(0..=8);
        mode INITIAL {
            token r"." => 99;
        }
    }
}

fn main() {}
