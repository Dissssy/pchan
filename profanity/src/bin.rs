use profanity::{replace_possible_profanity, Profanity};

pub fn main() {
    let known_bad = "fag faggot fag 💀 faggot fag fag faggot
faggot fag fag faggot fag!!!!! greaser faggot fag fag :D
i hate fags!!!"
        .to_owned();
    // let known_bad = "you are a faggot".to_owned();
    let profanity = Profanity::load_csv(env!("PROFANITY_PATH")).expect("failed to load profanity");

    println!(
        "|{}|",
        replace_possible_profanity(known_bad, &profanity, || { "💀".to_owned() })
    );
}
