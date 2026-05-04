use scnr2::scanner;

scanner! {
    BadScanner {
        mode INITIAL {
            token capture("#", n) => 1;
        }
    }
}

fn main() {}
