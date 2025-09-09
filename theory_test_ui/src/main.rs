use dioxus::prelude::*;
use rand::{
    seq::{IndexedRandom, SliceRandom},
    SeedableRng,
};
use theory_test_parser::question_parser::{ExamQuestions, Question};

const MAIN_CSS: Asset = asset!("/assets/main.css");

#[derive(Routable, Clone)]
pub enum Route {
    #[route("/")]
    MainPage,
    #[route("/real_exam")]
    RealExam,
    #[route("/pratice_exam?:num_questions")]
    PracticeExam { num_questions: usize },
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        Router::<Route> {}
    }
}

#[component]
pub fn MainPage() -> Element {
    let mut num_questions = use_signal(|| 30);
    let nav = navigator();

    rsx! {
        div {
            display: "flex",
            flex_direction: "column",
            align_items: "center",
            justify_content: "center",
            text_align: "center",
            div {
                h1 { "מבחן תאוריה" }
            }

            div { dir: "rtl",
                button {

                    onclick: move |_| {
                        nav.push(Route::PracticeExam {
                            num_questions: num_questions.read().clone(),
                        });
                    },
                    class: "button-primary",
                    "מבחן תרגול"

                }
                "מספר שאלות"
                input {
                    oninput: move |e| {
                        *num_questions.write() = e.value().parse().unwrap();
                    },
                    r#type: "number",
                    value: num_questions,
                    min: "1",

                }
            }

            div {
                button {
                    onclick: move |_| {
                        nav.push(Route::RealExam);
                    },
                    class: "button-primary",
                    "מבחן אמיתי"
                }
            }
        }
    }
}

#[component]
pub fn RealExam() -> Element {
    let exam_questions =
        ExamQuestions::parse_from_xlsx(include_bytes!("../../theory_test_parser/test.xlsx"))
            .unwrap();
    rsx! {
        Exam { exam_questions: Unchangable(exam_questions), num_questions: 30 }
    }
}

/// A wrapper over a type which makes it unchangable in dioxus' eyes,
/// i.e. the prop will never be changed and the component is not expected to be updated externally.
struct Unchangable<T>(T);
impl<T> PartialEq for Unchangable<T> {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}
impl<T: Clone> Clone for Unchangable<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[component]
pub fn PracticeExam(num_questions: usize) -> Element {
    let exam_questions =
        ExamQuestions::parse_from_xlsx(include_bytes!("../../theory_test_parser/test.xlsx"))
            .unwrap();
    rsx! {
        Exam { exam_questions: Unchangable(exam_questions), num_questions }
    }
}

#[component]
fn Exam(exam_questions: Unchangable<ExamQuestions>, num_questions: usize) -> Element {
    // it's in a signal to prevent regenerating a new rng.
    let mut rng = use_signal(|| rand_pcg::Pcg64::from_os_rng());
    let mut show_correct_answers = use_signal(|| false);
    let b_questions = exam_questions
        .0
        .questions
        .iter()
        .filter(|s| {
            s.license_classes
                .contains(&theory_test_parser::question_parser::LicenseClass::B)
        })
        .cloned()
        .collect::<Vec<Question>>();
    let questions = use_memo(move || {
        let mut questions = b_questions
            .choose_multiple(&mut rng(), num_questions)
            .cloned()
            .collect::<Vec<Question>>();
        // shuffle questions
        for question in questions.iter_mut() {
            let correct_answer_str = question
                .answers
                .possible_answers
                .get(question.answers.correct_answer)
                .unwrap()
                .clone();
            question.answers.possible_answers.shuffle(&mut rng());
            question.answers.correct_answer = question
                .answers
                .possible_answers
                .iter()
                .enumerate()
                .find(|(_, q)| q.as_str() == correct_answer_str.as_str())
                .unwrap()
                .0;
        }
        questions
    });

    let mut user_selections = Vec::with_capacity(num_questions);
    for _ in 1..=num_questions {
        // NOTE: THIS IS ONLY FINE BECAUSE NUM_QUESTIONS DOES NOT CHANGE WITHIN THE COMPONENT.
        user_selections.push(use_signal(|| None));
    }
    let user_selections = std::rc::Rc::new(user_selections);
    let user_selections_clone = user_selections.clone();
    let questions_clone = questions.clone();
    let correct_answers = use_memo(move || {
        let mut sum = 0;
        for (question, user_selection) in questions_clone.iter().zip(user_selections_clone.iter()) {
            if user_selection().is_some_and(|s| s == question.answers.correct_answer) {
                sum += 1;
            }
        }
        sum
    });

    rsx! {

        div { dir: "rtl", class: "exam-body",
            for (question_num , (question , user_selection)) in questions.iter().zip(user_selections.iter().cloned()).enumerate() {
                div { margin_bottom: "100px",
                    ExamQuestion {
                        question: question.clone(),
                        show_correct_answer: show_correct_answers.read().clone(),
                        user_selection,
                        question_num: question_num + 1,
                        show_question_num: true,
                        use_canonical_question_num: false,
                    }

                }
            }
            button {
                class: "button-primary",
                font_size: "large",
                onclick: move |_| {
                    *show_correct_answers.write() = true;
                },
                "בדוק מבחן"
            }
            if show_correct_answers() {
                div {
                    button {
                        class: "button-primary",
                        font_size: "large",
                        onclick: move |_| {
                            // reset all states
                            rng.set(rand_pcg::Pcg64::from_os_rng());
                            for mut signal in user_selections.iter().cloned() {
                                signal.set(None);
                            }
                            show_correct_answers.set(false);
                            document::eval(r#"window.scrollTo(0, 0);"#);

                        },
                        "התחל מבחן מחדש"

                    }
                }
                div { {format!("שאלות נכונות {}/{}", correct_answers(), num_questions)} }

            }
        }
    }
}

#[component]
pub fn ExamQuestion(
    question: Question,
    show_correct_answer: bool,
    mut user_selection: Signal<Option<usize>>,
    question_num: usize,
    show_question_num: bool,
    use_canonical_question_num: bool,
) -> Element {
    let correct_color = if show_correct_answer { "green" } else { "" };
    let wrong_color = if show_correct_answer { "red" } else { "" };
    let question_str = if show_question_num {
        if use_canonical_question_num {
            question.question
        } else {
            format!("{}. ", question_num) + &question.question.as_str()[6..]
        }
    } else {
        question.question.as_str()[6..].to_string()
    };

    rsx! {
        div { class: "question-container",
            h1 {
                class: "question",
                 {question_str} }
            div {
                if let Some(img) = question.image_url {
                    img { src: img, margin_bottom: "20px" }
                }

                div { class: "answers-container",
                    for (answer_num , answer) in question.answers.possible_answers.iter().enumerate() {

                        {
                            // for some reason naming it color makes dx fmt get rid of it
                            let colorr = if answer_num == question.answers.correct_answer {
                                correct_color
                            } else if user_selection().is_some_and(|s| s == answer_num) {
                                wrong_color
                            } else {
                                ""
                            };
                            rsx! {
                                label { color: colorr, class: "answer",
                                    input {
                                        oninput: move |_| {
                                            user_selection.set(Some(answer_num));
                                        },
                                        r#type: "radio",
                                        class: "answer_input",
                                        id: format!("answer_input{}{}", question.num, answer_num),
                                        name: format!("{}", question.num),
                                        checked: user_selection() == Some(answer_num),
                                    }
                                    "{answer}"

                                }
                            }
                        }
                    }


                }

                div { class: "category",
                    {format!("קטגוריה: {}", question.category.as_str_he())}
                }

            }

        }
    }
}
