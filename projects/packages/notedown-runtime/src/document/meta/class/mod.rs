/// https://ctan.org/topic/class
#[derive(Debug)]
pub enum DocumentClass {
    Article(DocumentClassArticle),
}
#[derive(Debug)]
pub struct DocumentClassArticle {
    pub font_size: usize,
}

impl DocumentClass {
    pub fn parse(&mut self) {}
}

impl DocumentClassArticle {
    pub fn parse_short(&mut self) {}

    pub fn parse(&mut self) {}
}
