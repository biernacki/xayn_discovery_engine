// Copyright 2021 Xayn AG
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as
// published by the Free Software Foundation, version 3.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use std::{
    borrow::Cow,
    fs::File,
    io::{BufRead, BufReader},
};

use derive_more::{Deref, From};
use itertools::Itertools;
use lindera::{
    mode::Mode as JapaneseMode,
    tokenizer::{
        DictionaryConfig as JapaneseDictionaryConfig,
        Tokenizer as JapanesePreTokenizer,
        TokenizerConfig as JapanesePreTokenizerConfig,
    },
};
use ndarray::Array2;
use tokenizers::{
    decoders::wordpiece::WordPiece as WordPieceDecoder,
    models::wordpiece::{WordPiece as WordPieceModel, WordPieceBuilder},
    normalizers::bert::BertNormalizer,
    pre_tokenizers::bert::BertPreTokenizer,
    processors::bert::BertProcessing,
    utils::{
        padding::{PaddingDirection, PaddingParams, PaddingStrategy},
        truncation::{TruncationDirection, TruncationParams, TruncationStrategy},
    },
    Error,
    Model,
    TokenizerBuilder,
    TokenizerImpl,
};
use tract_onnx::prelude::{tvec, TVec, Tensor};

use crate::config::Config;

/// A pre-configured Bert tokenizer.
pub struct Tokenizer {
    japanese: Option<JapanesePreTokenizer>,
    bert: TokenizerImpl<
        WordPieceModel,
        BertNormalizer,
        BertPreTokenizer,
        BertProcessing,
        WordPieceDecoder,
    >,
}

/// The attention mask of the encoded sequence.
///
/// The attention mask is of shape `(1, token_size)`.
#[derive(Clone, Deref, From)]
pub(crate) struct AttentionMask(pub(crate) Array2<i64>);

/// The encoded sequence.
#[derive(Clone)]
pub struct Encoding {
    pub(crate) token_ids: Array2<i64>,
    pub(crate) attention_mask: Array2<i64>,
    pub(crate) type_ids: Array2<i64>,
}

impl Encoding {
    pub(crate) fn to_attention_mask(&self) -> AttentionMask {
        self.attention_mask.clone().into()
    }
}

impl From<Encoding> for TVec<Tensor> {
    fn from(encoding: Encoding) -> Self {
        tvec![
            encoding.token_ids.into(),
            encoding.attention_mask.into(),
            encoding.type_ids.into(),
        ]
    }
}

impl From<Encoding> for Vec<Array2<i64>> {
    fn from(encoding: Encoding) -> Self {
        vec![
            encoding.token_ids,
            encoding.attention_mask,
            encoding.type_ids,
        ]
    }
}

impl Tokenizer {
    /// Creates a tokenizer from a configuration.
    pub fn new<P>(config: &Config<P>) -> Result<Self, Error> {
        let japanese = config
            .extract::<String>("pre-tokenizer.path")
            .ok()
            .map(|mecab| {
                JapanesePreTokenizer::with_config(JapanesePreTokenizerConfig {
                    dictionary: JapaneseDictionaryConfig {
                        kind: None,
                        path: Some(config.dir.join(mecab)),
                    },
                    user_dictionary: None,
                    mode: JapaneseMode::Normal,
                })
            })
            .transpose()?;

        let vocab = BufReader::new(File::open(config.dir.join("vocab.txt"))?)
            .lines()
            .enumerate()
            .map(|(idx, word)| Ok((word?.trim().to_string(), u32::try_from(idx)?)))
            .collect::<Result<_, Error>>()?;
        let model = WordPieceBuilder::new()
            .vocab(vocab)
            .unk_token(config.extract("tokenizer.tokens.unknown")?)
            .continuing_subword_prefix(config.extract("tokenizer.tokens.continuation")?)
            .max_input_chars_per_word(config.extract("tokenizer.max-chars")?)
            .build()?;
        let normalizer = BertNormalizer::new(
            config.extract("tokenizer.cleanse-text")?,
            false,
            Some(config.extract("tokenizer.cleanse-accents")?),
            config.extract("tokenizer.lower-case")?,
        );
        let sep_token = config.extract::<String>("tokenizer.tokens.separation")?;
        let sep_id = model.token_to_id(&sep_token).ok_or("missing sep token")?;
        let cls_token = config.extract::<String>("tokenizer.tokens.class")?;
        let cls_id = model.token_to_id(&cls_token).ok_or("missing cls token")?;
        let post_processor = BertProcessing::new((sep_token, sep_id), (cls_token, cls_id));
        let pad_token = config.extract::<String>("tokenizer.tokens.padding")?;
        let padding = PaddingParams {
            strategy: PaddingStrategy::Fixed(config.token_size),
            direction: PaddingDirection::Right,
            pad_to_multiple_of: None,
            pad_id: model.token_to_id(&pad_token).ok_or("missing pad token")?,
            pad_type_id: 0,
            pad_token,
        };
        let truncation = TruncationParams {
            direction: TruncationDirection::Right,
            max_length: config.token_size,
            strategy: TruncationStrategy::LongestFirst,
            stride: 0,
        };
        let decoder = WordPieceDecoder::new(config.extract("tokenizer.tokens.continuation")?, true);

        let bert = TokenizerBuilder::new()
            .with_model(model)
            .with_normalizer(Some(normalizer))
            .with_pre_tokenizer(Some(BertPreTokenizer))
            .with_post_processor(Some(post_processor))
            .with_padding(Some(padding))
            .with_truncation(Some(truncation))
            .with_decoder(Some(decoder))
            .build()?;

        Ok(Tokenizer { japanese, bert })
    }

    /// Encodes the sequence.
    ///
    /// The encoding is in correct shape for the model.
    pub fn encode(&self, sequence: impl AsRef<str>) -> Result<Encoding, Error> {
        #[allow(unstable_name_collisions)]
        let sequence = if let Some(japanese) = &self.japanese {
            japanese
                .tokenize(sequence.as_ref())?
                .into_iter()
                .map(|token| token.text)
                .intersperse(" ".into())
                .collect::<String>()
                .into()
        } else {
            Cow::Borrowed(sequence.as_ref())
        };

        let encoding = self.bert.encode(sequence, true)?;
        let array_from =
            |slice: &[u32]| Array2::from_shape_fn((1, slice.len()), |(_, i)| i64::from(slice[i]));

        Ok(Encoding {
            token_ids: array_from(encoding.get_ids()),
            attention_mask: array_from(encoding.get_attention_mask()),
            type_ids: array_from(encoding.get_type_ids()),
        })
    }
}

#[cfg(test)]
mod tests {
    use ndarray::ArrayView;
    use xayn_ai_test_utils::asset::{sjbert, smbert_mocked};

    use super::*;

    fn tokenizer(token_size: usize) -> Tokenizer {
        let config = Config::new(smbert_mocked().unwrap())
            .unwrap()
            .with_token_size(token_size)
            .unwrap();
        Tokenizer::new(&config).unwrap()
    }

    #[test]
    fn test_new_multi() {
        let multi = tokenizer(42);
        assert!(multi.japanese.is_none());
        assert!(multi.bert.get_normalizer().is_some());
        assert!(multi.bert.get_pre_tokenizer().is_some());
        assert!(multi.bert.get_post_processor().is_some());
        assert!(multi.bert.get_padding().is_some());
        assert!(multi.bert.get_truncation().is_some());
        assert!(multi.bert.get_decoder().is_some());
    }

    #[test]
    fn test_new_japan() {
        let config = Config::new(sjbert().unwrap())
            .unwrap()
            .with_token_size(42)
            .unwrap();
        let japan = Tokenizer::new(&config).unwrap();
        assert!(japan.japanese.is_some());
        assert!(japan.bert.get_normalizer().is_some());
        assert!(japan.bert.get_pre_tokenizer().is_some());
        assert!(japan.bert.get_post_processor().is_some());
        assert!(japan.bert.get_padding().is_some());
        assert!(japan.bert.get_truncation().is_some());
        assert!(japan.bert.get_decoder().is_some());
    }

    #[test]
    fn test_encode_short() {
        let shape = (1, 20);
        let encoding = tokenizer(shape.1)
            .encode("These are normal, common EMBEDDINGS.")
            .unwrap();
        assert_eq!(
            encoding.token_ids,
            ArrayView::from_shape(
                shape,
                &[2, 4538, 2128, 8561, 1, 6541, 69469, 2762, 5, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            )
            .unwrap(),
        );
        assert_eq!(
            encoding.attention_mask,
            ArrayView::from_shape(
                shape,
                &[1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            )
            .unwrap(),
        );
        assert_eq!(
            encoding.type_ids,
            ArrayView::from_shape(
                shape,
                &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            )
            .unwrap(),
        );
    }

    #[test]
    fn test_encode_long() {
        let shape = (1, 10);
        let encoding = tokenizer(shape.1)
            .encode("These are normal, common EMBEDDINGS.")
            .unwrap();
        assert_eq!(
            encoding.token_ids,
            ArrayView::from_shape(shape, &[2, 4538, 2128, 8561, 1, 6541, 69469, 2762, 5, 3])
                .unwrap(),
        );
        assert_eq!(
            encoding.attention_mask,
            ArrayView::from_shape(shape, &[1, 1, 1, 1, 1, 1, 1, 1, 1, 1]).unwrap(),
        );
        assert_eq!(
            encoding.type_ids,
            ArrayView::from_shape(shape, &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).unwrap(),
        );
    }

    #[test]
    fn test_encode_troublemakers() {
        let shape = (1, 15);
        let encoding = tokenizer(shape.1)
            .encode("for “life-threatening storm surge” according")
            .unwrap();
        assert_eq!(
            encoding.token_ids,
            ArrayView::from_shape(
                shape,
                &[2, 1665, 1, 3902, 1, 83775, 11123, 41373, 1, 7469, 3, 0, 0, 0, 0],
            )
            .unwrap(),
        );
        assert_eq!(
            encoding.attention_mask,
            ArrayView::from_shape(shape, &[1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0]).unwrap(),
        );
        assert_eq!(
            encoding.type_ids,
            ArrayView::from_shape(shape, &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).unwrap(),
        );
    }
}