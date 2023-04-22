pub mod board_title;
pub mod lazy_post;
pub mod new_post_box;
pub mod post_container;
pub mod startswith_class;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Reply {
    pub post_number: u64,
    pub board_discrim: String,
    pub external: bool,
}

impl Reply {
    pub fn from_str(s: &str, board: &str) -> Result<Self, ()> {
        // >>{post_number}
        // or
        // >>>/{board_discrim}/{post_number}

        // THIS BOARD POST DOES NOT HAVE A /

        let mut split = s.split('/');
        let first = split.next();
        let second = split.next();
        let third = split.next();
        let fourth = split.next();

        match (first, second, third, fourth) {
            (Some(">>>"), Some(b), Some(n), None) => {
                let board_discrim = b.to_owned();
                let post_number = n.parse::<u64>().map_err(|_| ())?;
                Ok(Reply {
                    post_number,
                    board_discrim,
                    external: true,
                })
            }
            _ => {
                let mut split = s.split(">>");
                let first = split.next();
                let second = split.next();
                let third = split.next();

                match (first, second, third) {
                    (Some(""), Some(n), None) => {
                        let post_number = n.parse::<u64>().map_err(|_| ())?;
                        Ok(Reply {
                            post_number,
                            board_discrim: board.to_owned(),
                            external: false,
                        })
                    }
                    _ => Err(()),
                }
            }
        }
    }
    pub fn link(&self) -> String {
        format!(
            "/{board_discrim}/thread/{post_number}",
            board_discrim = self.board_discrim,
            post_number = self.post_number
        )
    }
    pub fn text(&self) -> String {
        if self.external {
            format!(
                ">>>/{board_discrim}/{post_number}",
                board_discrim = self.board_discrim,
                post_number = self.post_number
            )
        } else {
            format!(">>{post_number}", post_number = self.post_number)
        }
    }
}
