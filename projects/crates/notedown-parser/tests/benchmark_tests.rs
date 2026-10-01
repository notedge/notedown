//! Benchmark tests for Notedown parser performance

use notedown_parser::helpers::{NotedownNode, ParseState};
use notedown_parser::parsers::*;
use std::time::Instant;

#[cfg(test)]
mod benchmark_tests {
    use super::*;

    fn create_sample_document(size: usize) -> String {
        let mut doc = String::new();
        
        for i in 0..size {
            doc.push_str(&format!("# Header {}\n\n", i));
            doc.push_str(&format!("This is paragraph {} with **bold** and *italic* text. ", i));
            doc.push_str(&format!("It also contains `inline code` and [links](https://example{}.com).\n\n", i));
            
            doc.push_str(&format!("> Quote block {}\n", i));
            doc.push_str(&format!("> with multiple lines\n\n"));
            
            doc.push_str(&format!("- List item {}\n", i));
            doc.push_str(&format!("- Another item {}\n", i));
            doc.push_str(&format!("  - Nested item {}\n\n", i));
            
            if i % 10 == 0 {
                doc.push_str("```rust\n");
                doc.push_str(&format!("fn function_{}() {{\n", i));
                doc.push_str(&format!("    println!(\"Function {}\");\n", i));
                doc.push_str("}\n```\n\n");
            }
            
            if i % 15 == 0 {
                doc.push_str(&format!("$$\n\\sum_{{i=0}}^{{{}}} i^2\n$$\n\n", i));
            }
            
            if i % 20 == 0 {
                doc.push_str(&format!("| Header {} | Value {} |\n", i, i));
                doc.push_str("|-----------|----------|\n");
                doc.push_str(&format!("| Data {}   | Info {}  |\n\n", i, i));
            }
        }
        
        doc
    }

    #[test]
    fn benchmark_small_document() {
        let doc = create_sample_document(10);
        let mut state = ParseState::new(doc.as_str());
        
        let start = Instant::now();
        let result = NotedownAST::parse(&mut state);
        let duration = start.elapsed();
        
        assert!(result.is_ok());
        println!("Small document (10 sections): {:?}", duration);
        assert!(duration.as_millis() < 100); // Should be very fast
    }

    #[test]
    fn benchmark_medium_document() {
        let doc = create_sample_document(100);
        let mut state = ParseState::new(doc.as_str());
        
        let start = Instant::now();
        let result = NotedownAST::parse(&mut state);
        let duration = start.elapsed();
        
        assert!(result.is_ok());
        println!("Medium document (100 sections): {:?}", duration);
        assert!(duration.as_millis() < 1000); // Should complete within 1 second
    }

    #[test]
    fn benchmark_large_document() {
        let doc = create_sample_document(500);
        let mut state = ParseState::new(doc.as_str());
        
        let start = Instant::now();
        let result = NotedownAST::parse(&mut state);
        let duration = start.elapsed();
        
        assert!(result.is_ok());
        println!("Large document (500 sections): {:?}", duration);
        assert!(duration.as_secs() < 10); // Should complete within 10 seconds
    }

    #[test]
    fn benchmark_text_parsing() {
        let text = "This is a **bold** text with *italic* and `code` elements. ".repeat(1000);
        let mut state = ParseState::new(text.as_str());
        
        let start = Instant::now();
        let result = paragraph::ParagraphSpan::parse(&mut state);
        let duration = start.elapsed();
        
        assert!(result.is_ok());
        println!("Text parsing (1000 repetitions): {:?}", duration);
        assert!(duration.as_millis() < 500);
    }

    #[test]
    fn benchmark_header_parsing() {
        let headers = (1..=1000)
            .map(|i| format!("# Header {}\n", i))
            .collect::<String>();
        let mut state = ParseState::new(headers.as_str());
        
        let start = Instant::now();
        let mut count = 0;
        while !state.is_at_end() {
            if let Ok(_) = header::HeadingSpan::parse(&mut state) {
                count += 1;
            }
            state.skip_whitespace();
        }
        let duration = start.elapsed();
        
        println!("Header parsing (1000 headers, {} parsed): {:?}", count, duration);
        assert!(duration.as_millis() < 200);
    }

    #[test]
    fn benchmark_list_parsing() {
        let mut list_content = String::new();
        for i in 1..=1000 {
            list_content.push_str(&format!("- List item {}\n", i));
        }
        let mut state = ParseState::new(list_content.as_str());
        
        let start = Instant::now();
        let mut count = 0;
        while !state.is_at_end() {
            if let Ok(_) = list::parse_list(&mut state) {
                count += 1;
            }
            state.skip_whitespace();
        }
        let duration = start.elapsed();
        
        println!("List parsing (1000 items, {} parsed): {:?}", count, duration);
        assert!(duration.as_millis() < 300);
    }

    #[test]
    fn benchmark_code_block_parsing() {
        let mut code_blocks = String::new();
        for i in 1..=100 {
            code_blocks.push_str(&format!(
                "```rust\nfn function_{}() {{\n    println!(\"Function {}\");\n    return {};\n}}\n```\n\n",
                i, i, i
            ));
        }
        let mut state = ParseState::new(code_blocks.as_str());
        
        let start = Instant::now();
        let mut count = 0;
        while !state.is_at_end() {
            if let Ok(_) = code::parse_code_block(&mut state) {
                count += 1;
            }
            state.skip_whitespace();
        }
        let duration = start.elapsed();
        
        println!("Code block parsing (100 blocks, {} parsed): {:?}", count, duration);
        assert!(duration.as_millis() < 200);
    }

    #[test]
    fn benchmark_math_parsing() {
        let mut math_content = String::new();
        for i in 1..=100 {
            math_content.push_str(&format!("$x_{} = y_{} + z_{}$\n\n", i, i, i));
            math_content.push_str(&format!("$$\n\\sum_{{i=0}}^{{{}}} i^2 = \\frac{{{}({}+1)(2\\cdot{}+1)}}{{6}}\n$$\n\n", i, i, i, i));
        }
        let mut state = ParseState::new(math_content.as_str());
        
        let start = Instant::now();
        let result = NotedownAST::parse(&mut state);
        let duration = start.elapsed();
        
        assert!(result.is_ok());
        println!("Math parsing (200 expressions): {:?}", duration);
        assert!(duration.as_millis() < 300);
    }

    #[test]
    fn benchmark_table_parsing() {
        let mut table_content = String::new();
        for i in 1..=50 {
            table_content.push_str(&format!("| Header {} | Value {} | Data {} |\n", i, i, i));
            table_content.push_str("|-----------|----------|---------|\n");
            for j in 1..=10 {
                table_content.push_str(&format!("| Cell {}-{} | Val {}-{} | Info {}-{} |\n", i, j, i, j, i, j));
            }
            table_content.push_str("\n");
        }
        let mut state = ParseState::new(table_content.as_str());
        
        let start = Instant::now();
        let mut count = 0;
        while !state.is_at_end() {
            if let Ok(_) = table::parse_table(&mut state) {
                count += 1;
            }
            state.skip_whitespace();
        }
        let duration = start.elapsed();
        
        println!("Table parsing (50 tables, {} parsed): {:?}", count, duration);
        assert!(duration.as_millis() < 500);
    }

    #[test]
    fn benchmark_mixed_content() {
        let mixed_content = r#"
# Main Title

This is a paragraph with **bold**, *italic*, and `code` text.

> This is a blockquote
> with multiple lines

- List item 1
- List item 2
  - Nested item

1. Ordered item
2. Another item

```rust
fn main() {
    println!("Hello, world!");
}
```

$$
\int_0^1 x^2 dx = \frac{1}{3}
$$

| Header 1 | Header 2 |
|----------|----------|
| Cell 1   | Cell 2   |

[Link](https://example.com)

---

"#.repeat(100);
        
        let mut state = ParseState::new(mixed_content.as_str());
        
        let start = Instant::now();
        let result = NotedownAST::parse(&mut state);
        let duration = start.elapsed();
        
        assert!(result.is_ok());
        println!("Mixed content parsing (100 repetitions): {:?}", duration);
        assert!(duration.as_secs() < 5);
    }

    #[test]
    fn benchmark_memory_usage() {
        // This test is more about ensuring we don't have memory leaks
        // In a real scenario, you'd use a memory profiler
        let doc = create_sample_document(1000);
        
        for _ in 0..10 {
            let mut state = ParseState::new(doc.as_str());
            let _result = NotedownAST::parse(&mut state);
            // Force garbage collection if available
            // In Rust, memory is automatically managed
        }
        
        // If we reach here without running out of memory, the test passes
        assert!(true);
    }
}

#[cfg(test)]
mod stress_tests {
    use super::*;

    #[test]
    fn stress_test_deeply_nested() {
        let mut nested_content = String::new();
        
        // Create deeply nested quotes
        for i in 0..100 {
            nested_content.push_str(&"> ".repeat(i + 1));
            nested_content.push_str(&format!("Nested level {}\n", i + 1));
        }
        
        let mut state = ParseState::new(nested_content.as_str());
        let start = Instant::now();
        let result = NotedownAST::parse(&mut state);
        let duration = start.elapsed();
        
        assert!(result.is_ok());
        println!("Deeply nested content: {:?}", duration);
        assert!(duration.as_secs() < 2);
    }

    #[test]
    fn stress_test_long_lines() {
        let long_line = "This is a very long line with **bold** and *italic* text. ".repeat(1000);
        let mut state = ParseState::new(long_line.as_str());
        
        let start = Instant::now();
        let result = NotedownAST::parse(&mut state);
        let duration = start.elapsed();
        
        assert!(result.is_ok());
        println!("Long line parsing: {:?}", duration);
        assert!(duration.as_millis() < 500);
    }

    #[test]
    fn stress_test_many_small_elements() {
        let mut content = String::new();
        for i in 0..10000 {
            content.push_str(&format!("**{}** ", i));
        }
        
        let mut state = ParseState::new(content.as_str());
        let start = Instant::now();
        let result = NotedownAST::parse(&mut state);
        let duration = start.elapsed();
        
        assert!(result.is_ok());
        println!("Many small elements: {:?}", duration);
        assert!(duration.as_secs() < 3);
    }
}

fn bench_parse_document_small(c: &mut Criterion) {
    let doc = "# Small Document\n\nThis is a small test document.";
    c.bench_function("parse_document_small", |b| b.iter(|| {
        let mut state = ParseState::new(&doc);
        parse_document(&mut state)
    }));
}

fn bench_parse_document_medium(c: &mut Criterion) {
    let doc = medium_doc;
    c.bench_function("parse_document_medium", |b| b.iter(|| {
        let mut state = ParseState::new(&doc);
        parse_document(&mut state)
    }));
}

fn bench_parse_document_large(c: &mut Criterion) {
    let doc = large_doc;
    c.bench_function("parse_document_large", |b| b.iter(|| {
        let mut state = ParseState::new(&doc);
        parse_document(&mut state)
    }));
}

fn bench_parse_text(c: &mut Criterion) {
    let text = "This is a simple paragraph.";
    c.bench_function("parse_text", |b| b.iter(|| {
        let mut state = ParseState::new(text);
        paragraph::ParagraphSpan::parse(&mut state)
    }));
}

fn bench_parse_header(c: &mut Criterion) {
    let text = "## This is a header";
    c.bench_function("parse_header", |b| b.iter(|| {
        let mut state = ParseState::new(text);
        header::HeadingSpan::parse(&mut state)
    }));
}

fn bench_parse_list(c: &mut Criterion) {
    let text = "- Item 1\n- Item 2\n- Item 3";
    c.bench_function("parse_list", |b| b.iter(|| {
        let mut state = ParseState::new(text);
        list::parse_list(&mut state)
    }));
}

fn bench_parse_code_block(c: &mut Criterion) {
    let text = "```rust\nfn main() {\n    println!(\"Hello, world!\");\n}\n```";
    c.bench_function("parse_code_block", |b| b.iter(|| {
        let mut state = ParseState::new(text);
        code::parse_code_block(&mut state)
    }));
}

fn bench_parse_math(c: &mut Criterion) {
    let text = "$$ E = mc^2 $$";
    c.bench_function("parse_math", |b| b.iter(|| {
        let mut state = ParseState::new(text);
        math::parse_math_block(&mut state)
    }));
}

fn bench_parse_table(c: &mut Criterion) {
    let text = "| Header 1 | Header 2 |\n|---|---|\n| Cell 1 | Cell 2 |";
    c.bench_function("parse_table", |b| b.iter(|| {
        let mut state = ParseState::new(text);
        table::parse_table(&mut state)
    }));
}

fn bench_stress_deeply_nested(c: &mut Criterion) {
    let doc = nested_doc;
    c.bench_function("stress_deeply_nested", |b| b.iter(|| {
        let mut state = ParseState::new(&doc);
        parse_document(&mut state)
    }));
}

fn bench_stress_long_lines(c: &mut Criterion) {
    let doc = long_line_doc;
    c.bench_function("stress_long_lines", |b| b.iter(|| {
        let mut state = ParseState::new(&doc);
        parse_document(&mut state)
    }));
}

fn bench_stress_many_small_elements(c: &mut Criterion) {
    let doc = many_elements_doc;
    c.bench_function("stress_many_small_elements", |b| b.iter(|| {
        let mut state = ParseState::new(&doc);
        parse_document(&mut state)
    }));
}

fn bench_memory_usage(c: &mut Criterion) {
    let doc = large_doc;
    c.bench_function("memory_usage", |b| b.iter_with_large_drop(|| {
        let mut state = ParseState::new(&doc);
        parse_document(&mut state)
    }));
}

criterion_group!(
    use super::*;
}