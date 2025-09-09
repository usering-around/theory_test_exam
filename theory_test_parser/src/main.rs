use theory_test_parser::question_parser::ExamQuestions;

fn main() {
    ExamQuestions::parse_from_xlsx_file("test.xlsx").unwrap();
}
