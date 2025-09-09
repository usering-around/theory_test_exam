use std::{
    fs::File,
    io::{BufReader, Read, Seek},
    path::Path,
};

use calamine::{DataType, Reader, Xlsx, XlsxError};
use quick_xml::events::Event;
use thiserror::Error;

#[derive(Clone, Copy, PartialEq)]
pub enum QuestionCategory {
    Safety,
    TrafficLaws,
    RoadSigns,
    CarKnowledge,
}

impl QuestionCategory {
    const SAFETY_HE: &str = "בטיחות";
    const TRAFFIC_LAWS_HE: &str = "חוקי התנועה";
    const CAR_KNOWLEDGE_HE: &str = "הכרת הרכב";
    const ROAD_SIGNS_HE: &str = "תמרורים";
    pub fn from_str_he(str: &str) -> Option<Self> {
        Some(match str {
            Self::SAFETY_HE => Self::Safety,
            Self::TRAFFIC_LAWS_HE => Self::TrafficLaws,
            Self::CAR_KNOWLEDGE_HE => Self::CarKnowledge,
            Self::ROAD_SIGNS_HE => Self::RoadSigns,
            _ => return None,
        })
    }

    pub fn as_str_he(&self) -> &'static str {
        match self {
            QuestionCategory::Safety => Self::SAFETY_HE,
            QuestionCategory::TrafficLaws => Self::TRAFFIC_LAWS_HE,
            QuestionCategory::CarKnowledge => Self::CAR_KNOWLEDGE_HE,
            QuestionCategory::RoadSigns => Self::ROAD_SIGNS_HE,
        }
    }
}

#[derive(PartialEq, Clone, Copy)]
pub enum LicenseClass {
    C1,
    C,
    D,
    A,
    B,
}

#[derive(Clone)]
pub struct Question {
    pub num: usize,
    /// the question
    pub question: String,
    /// possible answers
    pub answers: Answers,
    /// the category of the question
    pub category: QuestionCategory,
    /// the license classes this question is for
    pub license_classes: Vec<LicenseClass>,
    /// optional image url if there is any
    pub image_url: Option<String>,
}

impl PartialEq for Question {
    fn eq(&self, other: &Self) -> bool {
        self.num == other.num
    }
}

const POSSIBLE_ANSWERS_NUM: usize = 4;

#[derive(Clone)]
pub struct Answers {
    pub possible_answers: Vec<String>,
    pub correct_answer: usize,
}

fn parse_answers(xml: &str) -> (Answers, Vec<LicenseClass>, Option<String>) {
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut possible_answers = Vec::new();
    let mut license_classes = Vec::new();
    let mut correct_answer = 0;
    let mut image_url = None;
    loop {
        match reader.read_event() {
            Err(e) => println!("error: {}", e),
            Ok(Event::Eof) => break,
            Ok(Event::Text(text)) => {
                let text = String::from_utf8(text.to_vec()).unwrap();
                // there is only 4 answers per question
                if possible_answers.len() < POSSIBLE_ANSWERS_NUM {
                    possible_answers.push(text);
                } else if text.starts_with("|") && text.trim_end().ends_with("|") {
                    // we have the classes which this belong
                    if text.contains("«A»") {
                        license_classes.push(LicenseClass::A);
                    }
                    if text.contains("«В»") {
                        license_classes.push(LicenseClass::B);
                    }
                    if text.contains("«C1»") {
                        license_classes.push(LicenseClass::C);
                    }
                    if text.contains("«C»") {
                        license_classes.push(LicenseClass::C1);
                    }
                    if text.contains("«D»") {
                        license_classes.push(LicenseClass::D);
                    }
                }
            }
            Ok(Event::Start(start)) => {
                if start.name().0 == b"span" {
                    for attribute in start.attributes() {
                        if let Ok(attribute) = attribute {
                            if attribute.key.0 == b"id"
                                && attribute.value.starts_with(b"correctAnswer")
                            {
                                correct_answer = possible_answers.len();
                            }
                        }
                    }
                }
            }
            Ok(Event::Empty(tag)) => {
                if tag.name().0 == b"img" {
                    for attribute in tag.attributes() {
                        if let Ok(attribute) = attribute {
                            if attribute.key.0 == b"src" {
                                image_url =
                                    Some(String::from_utf8(attribute.value.to_vec()).unwrap())
                            }
                        }
                    }
                }
            }
            _ => (),
        }
    }
    (
        Answers {
            possible_answers,
            correct_answer,
        },
        license_classes,
        image_url,
    )
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Xlsx error: {}", .0)]
    Xlsx(#[from] XlsxError),

    #[error("Did not find description4 header (answers) in the xlsx file")]
    NoDescription4Header,
    #[error("Did not find title2 header (questions) in the xlsx file")]
    NoTitle2Header,
    #[error("Did not find category header in the xlsx file")]
    NoCategoryHeader,
}

#[derive(Clone)]
pub struct ExamQuestions {
    pub questions: Vec<Question>,
}

impl ExamQuestions {
    pub fn parse_from_workbook<RS: Read + Seek>(mut workbook: Xlsx<RS>) -> Result<Self> {
        let worksheets = workbook.worksheets();
        let mut questions = Vec::new();
        // we only expect one worksheet
        let (_sheet_name, sheet_data) = worksheets
            .first()
            .expect("There should be a sheet in the xlsx file...");

        let (answers_column, _column_name) = sheet_data
            .headers()
            .unwrap()
            .iter()
            .enumerate()
            .find(|(_, h)| h.as_str() == "description4")
            .ok_or(Error::NoDescription4Header)?;
        let (question_column, _column_name) = sheet_data
            .headers()
            .unwrap()
            .iter()
            .enumerate()
            .find(|(_, h)| h.as_str() == "title2")
            .ok_or(Error::NoTitle2Header)?;
        let (category_column, _column_name) = sheet_data
            .headers()
            .unwrap()
            .iter()
            .enumerate()
            .find(|(_, h)| h.as_str() == "category")
            .ok_or(Error::NoCategoryHeader)?;
        for row in sheet_data.rows().skip(1) {
            let question = &row[question_column]
                .as_string()
                .expect("question should be a string");
            let answers = &row[answers_column]
                .as_string()
                .expect("answers should be a string");
            let (answers, license_classes, image_url) = parse_answers(answers);
            let category = row[category_column]
                .as_string()
                .expect("category should be a string");
            let category = QuestionCategory::from_str_he(&category).unwrap();
            let num = question[0..=3].parse().unwrap();
            questions.push(Question {
                num,
                question: question.clone(),
                answers,
                license_classes,
                image_url,
                category,
            });
        }

        Ok(ExamQuestions { questions })
    }
    pub fn parse_from_xlsx(bytes: &[u8]) -> Result<Self> {
        let rs = BufReader::new(std::io::Cursor::new(bytes));
        let workbook = calamine::open_workbook_from_rs(rs)?;
        Self::parse_from_workbook(workbook)
    }
    /// Parse the exam questions from an Xlsx file.
    pub fn parse_from_xlsx_file(path: impl AsRef<Path>) -> Result<Self> {
        let workbook = calamine::open_workbook::<Xlsx<BufReader<File>>, _>(path)?;
        Self::parse_from_workbook(workbook)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn question_parse() {
        let question_xml = r#"<div dir="rtl" style="text-align: right"><ul><li><span id="correctAnswer0862">שאנו בקיאים בהפעלתו ובשימוש בו.</span></li><li><span>שברכב בוצעו הטיפולים הדרושים לתחזוקתו השוטפת.</span></li><li><span>שברכב נמצאים נורות ונתיכים (פיוזים) חלופיים.</span></li><li><span>שהדלק והשמנים הם מהסוג המתאים להפעלתו התקינה של הרכב.</span></li></ul><div style="padding-top: 4px;"><span><button type="button" onclick="var correctAnswer=document.getElementById('correctAnswer0862');correctAnswer.style.background='yellow'">הצג תשובה נכונה</button></span><br/><span style="float: left;">| «C1» | «C» | «D» | «A» | «1» | «В» | </span></div></div>"#;
        let (answers, license_classes, image_url) = parse_answers(question_xml);
        let possible_answers = answers.possible_answers;
        assert_eq!(possible_answers[0], r#"שאנו בקיאים בהפעלתו ובשימוש בו."#);
        assert_eq!(
            possible_answers[1],
            r#"שברכב בוצעו הטיפולים הדרושים לתחזוקתו השוטפת."#
        );
        assert_eq!(
            possible_answers[2],
            r#"שברכב נמצאים נורות ונתיכים (פיוזים) חלופיים."#
        );
        assert_eq!(
            possible_answers[3],
            r#"שהדלק והשמנים הם מהסוג המתאים להפעלתו התקינה של הרכב."#
        );
        assert_eq!(answers.correct_answer, 0);
        assert_eq!(image_url, None);
        assert!(license_classes.contains(&LicenseClass::A));
        assert!(license_classes.contains(&LicenseClass::B));
        assert!(license_classes.contains(&LicenseClass::C));
        assert!(license_classes.contains(&LicenseClass::C1));
        assert!(license_classes.contains(&LicenseClass::D));

        let question_xml = r#"<div dir="rtl" style="text-align: right"><ul><li><span id="correctAnswer0667">עצור לפני הצומת, אלא אם כן אינך יכול לעצור בבטחה.</span></li><li><span>היכון לנסיעה. מיד יתחלף האור ברמזור לירוק.</span></li><li><span>המשך בנסיעה. האור ברמזור יתחלף מיד לאור ירוק.</span></li><li><span>מותר לנסוע ישר, ימינה ושמאלה.</span></li></ul><img src="https://www.gov.il/BlobFolder/generalpage/tq_pic_02/he/TQ_PIC_3667.jpg" style="width: 100%; padding: 0pt; border: 0pt none; outline: 0pt none;" alt="yellow_traffic_light" title="yellow_traffic_light" /><div style="padding-top: 4px;"><span><button type="button" onclick="var correctAnswer=document.getElementById('correctAnswer0667');correctAnswer.style.background='yellow'">הצג תשובה נכונה</button></span><br/><span style="float: left;">| «C1» | «C» | «D» | «A» | «1» | «В» | </span></div></div>"#;
        let (answers, license_classes, image_url) = parse_answers(question_xml);
        let possible_answers = answers.possible_answers;
        assert_eq!(
            possible_answers[0],
            r#"עצור לפני הצומת, אלא אם כן אינך יכול לעצור בבטחה."#
        );
        assert_eq!(
            possible_answers[1],
            r#"היכון לנסיעה. מיד יתחלף האור ברמזור לירוק."#
        );
        assert_eq!(
            possible_answers[2],
            r#"המשך בנסיעה. האור ברמזור יתחלף מיד לאור ירוק."#
        );
        assert_eq!(possible_answers[3], r#"מותר לנסוע ישר, ימינה ושמאלה."#);
        assert_eq!(answers.correct_answer, 0);
        assert_eq!(
            image_url,
            Some(
                "https://www.gov.il/BlobFolder/generalpage/tq_pic_02/he/TQ_PIC_3667.jpg"
                    .to_string()
            )
        );
        assert!(license_classes.contains(&LicenseClass::A));
        assert!(license_classes.contains(&LicenseClass::B));
        assert!(license_classes.contains(&LicenseClass::C));
        assert!(license_classes.contains(&LicenseClass::C1));
        assert!(license_classes.contains(&LicenseClass::D));
    }
}
