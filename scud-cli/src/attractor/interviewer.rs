//! Human-in-the-loop interaction trait.

use async_trait::async_trait;

/// A question to present to a human.
pub struct Question {
    /// The question text.
    pub text: String,
    /// Available choices (may be empty for free-form).
    pub choices: Vec<String>,
    /// Default choice index (if any).
    pub default: Option<usize>,
}

/// A human's answer.
pub struct Answer {
    /// The selected choice text.
    pub text: String,
    /// The index of the selected choice (if from a choice list).
    pub index: Option<usize>,
}

/// Trait for presenting questions to humans and getting answers.
#[async_trait]
pub trait Interviewer: Send + Sync {
    /// Ask a question and wait for an answer.
    async fn ask(&self, question: Question) -> Answer;
}

/// Console-based interviewer using stdin/stdout.
pub struct ConsoleInterviewer;

#[async_trait]
impl Interviewer for ConsoleInterviewer {
    async fn ask(&self, question: Question) -> Answer {
        use std::io::{self, Write};

        println!("\n{}", question.text);
        for (i, choice) in question.choices.iter().enumerate() {
            let marker = if Some(i) == question.default {
                "*"
            } else {
                " "
            };
            println!("  {} [{}] {}", marker, i + 1, choice);
        }

        print!("> ");
        let _ = io::stdout().flush();

        let mut input = String::new();
        let _ = io::stdin().read_line(&mut input);
        let input = input.trim();

        // Try parsing as a number (1-indexed choice)
        if let Ok(num) = input.parse::<usize>() {
            if num >= 1 && num <= question.choices.len() {
                return Answer {
                    text: question.choices[num - 1].clone(),
                    index: Some(num - 1),
                };
            }
        }

        // Free-form text answer
        Answer {
            text: input.to_string(),
            index: None,
        }
    }
}

/// Auto-approve interviewer that always selects the first option.
///
/// Used for `--headless` mode where no human is available.
pub struct AutoApproveInterviewer;

#[async_trait]
impl Interviewer for AutoApproveInterviewer {
    async fn ask(&self, question: Question) -> Answer {
        let (text, index) = if let Some(default) = question.default {
            (question.choices[default].clone(), Some(default))
        } else if !question.choices.is_empty() {
            (question.choices[0].clone(), Some(0))
        } else {
            ("yes".to_string(), None)
        };

        Answer { text, index }
    }
}

/// Queue-based interviewer with pre-filled answers (for testing).
pub struct QueueInterviewer {
    answers: std::sync::Mutex<Vec<Answer>>,
}

impl QueueInterviewer {
    /// Create a new queue interviewer with pre-filled answers.
    pub fn new(answers: Vec<Answer>) -> Self {
        Self {
            answers: std::sync::Mutex::new(answers),
        }
    }

    /// Create from a list of string answers.
    pub fn from_strings(answers: Vec<&str>) -> Self {
        Self::new(
            answers
                .into_iter()
                .map(|s| Answer {
                    text: s.to_string(),
                    index: None,
                })
                .collect(),
        )
    }
}

#[async_trait]
impl Interviewer for QueueInterviewer {
    async fn ask(&self, question: Question) -> Answer {
        let mut queue = self.answers.lock().unwrap();
        if queue.is_empty() {
            // Fallback: auto-approve
            if !question.choices.is_empty() {
                Answer {
                    text: question.choices[0].clone(),
                    index: Some(0),
                }
            } else {
                Answer {
                    text: "yes".to_string(),
                    index: None,
                }
            }
        } else {
            queue.remove(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_auto_approve_first_choice() {
        let interviewer = AutoApproveInterviewer;
        let answer = interviewer
            .ask(Question {
                text: "Choose".into(),
                choices: vec!["A".into(), "B".into()],
                default: None,
            })
            .await;
        assert_eq!(answer.text, "A");
        assert_eq!(answer.index, Some(0));
    }

    #[tokio::test]
    async fn test_auto_approve_default() {
        let interviewer = AutoApproveInterviewer;
        let answer = interviewer
            .ask(Question {
                text: "Choose".into(),
                choices: vec!["A".into(), "B".into()],
                default: Some(1),
            })
            .await;
        assert_eq!(answer.text, "B");
        assert_eq!(answer.index, Some(1));
    }

    #[tokio::test]
    async fn test_queue_interviewer() {
        let interviewer = QueueInterviewer::from_strings(vec!["first", "second"]);

        let a1 = interviewer
            .ask(Question {
                text: "Q1?".into(),
                choices: vec![],
                default: None,
            })
            .await;
        assert_eq!(a1.text, "first");

        let a2 = interviewer
            .ask(Question {
                text: "Q2?".into(),
                choices: vec![],
                default: None,
            })
            .await;
        assert_eq!(a2.text, "second");
    }

    #[tokio::test]
    async fn test_queue_interviewer_fallback() {
        let interviewer = QueueInterviewer::from_strings(vec![]);
        let answer = interviewer
            .ask(Question {
                text: "Q?".into(),
                choices: vec!["option".into()],
                default: None,
            })
            .await;
        assert_eq!(answer.text, "option");
    }
}
