// Example code that deserializes and serializes the model.
// extern crate serde;
// #[macro_use]
// extern crate serde_derive;
// extern crate serde_json;
//
// use generated_module::[object Object];
//
// fn main() {
//     let json = r#"{"answer": 42}"#;
//     let model: [object Object] = serde_json::from_str(&json).unwrap();
// }
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct GuestTokenFetchResponse {
    pub(crate) guest_token: String,
}

#[derive(Deserialize)]
pub struct TwitterUserLookup {
    pub(crate) id: String,
}

#[derive(Deserialize)]
pub struct TwitterUserLookupResponse {
    pub(crate) data: TwitterUserLookup,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterPostMetadata {
    pub(crate) language: Option<String>,
    pub(crate) like_count: Option<i64>,
    pub(crate) retweet_count: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterImageMetadata {
    pub(crate) width: i64,
    pub(crate) height: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Twitter {
    #[serde(rename = "globalObjects")]
    pub(crate) global_objects: GlobalObjects,
    pub(crate) timeline: Timeline,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GlobalObjects {
    pub(crate) tweets: HashMap<String, TweetValue>,
    pub(crate) users: HashMap<String, User>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TopicValue {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) following: bool,
    pub(crate) description: String,
    pub(crate) not_interested: bool,
    pub(crate) icon_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TweetValue {
    pub(crate) created_at: String,
    pub(crate) id_str: String,
    pub(crate) full_text: Option<String>,
    pub(crate) display_text_range: Vec<i64>,
    pub(crate) entities: TweetEntities,
    pub(crate) source: Option<String>,
    pub(crate) user_id_str: String,
    pub(crate) retweeted_status_id_str: Option<String>,
    pub(crate) retweet_count: Option<i64>,
    pub(crate) favorite_count: Option<i64>,
    pub(crate) conversation_id_str: Option<String>,
    pub(crate) lang: Option<String>,
    pub(crate) is_quote_status: Option<bool>,
    pub(crate) quoted_status_id_str: Option<String>,
    pub(crate) quoted_status_permalink: Option<QuotedStatusPermalink>,
    pub(crate) in_reply_to_status_id_str: Option<String>,
    pub(crate) in_reply_to_user_id_str: Option<String>,
    pub(crate) in_reply_to_screen_name: Option<String>,
    pub(crate) extended_entities: Option<ExtendedEntities>,
    pub(crate) possibly_sensitive_editable: Option<bool>,
    pub(crate) self_thread: Option<SelfThread>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TweetEntities {
    pub(crate) user_mentions: Option<Vec<UserMention>>,
    pub(crate) media: Option<Vec<EntitiesMedia>>,
    pub(crate) urls: Option<Vec<UrlElement>>,
    pub(crate) hashtags: Option<Vec<Hashtag>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Hashtag {
    pub(crate) text: String,
    pub(crate) indices: Vec<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EntitiesMedia {
    pub(crate) id_str: String,
    pub(crate) indices: Vec<i64>,
    pub(crate) media_url: String,
    pub(crate) media_url_https: String,
    pub(crate) url: String,
    pub(crate) display_url: String,
    pub(crate) expanded_url: String,
    #[serde(rename = "type")]
    pub(crate) media_type: Type,
    pub(crate) original_info: OriginalInfo,
    pub(crate) sizes: Sizes,
    pub(crate) media_key: Option<String>,
    pub(crate) ext: Option<PurpleExt>,
    pub(crate) source_status_id_str: Option<String>,
    pub(crate) source_user_id_str: Option<String>,
    pub(crate) video_info: Option<PurpleVideoInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PurpleExt {
    #[serde(rename = "mediaStats")]
    pub(crate) media_stats: PurpleMediaStats,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PurpleMediaStats {
    pub(crate) r: REnum,
    pub(crate) ttl: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OriginalInfo {
    pub(crate) width: i64,
    pub(crate) height: i64,
    pub(crate) focus_rects: Option<Vec<FocusRect>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FocusRect {
    pub(crate) x: i64,
    pub(crate) y: i64,
    pub(crate) h: i64,
    pub(crate) w: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Sizes {
    pub(crate) small: Large,
    pub(crate) medium: Large,
    pub(crate) thumb: Large,
    pub(crate) large: Large,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Large {
    pub(crate) w: i64,
    pub(crate) h: i64,
    pub(crate) resize: Resize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PurpleVideoInfo {
    pub(crate) aspect_ratio: Vec<i64>,
    pub(crate) variants: Vec<Variant>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Variant {
    pub(crate) bitrate: Option<i64>,
    pub(crate) content_type: ContentType,
    pub(crate) url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UrlElement {
    pub(crate) url: String,
    pub(crate) expanded_url: String,
    pub(crate) display_url: String,
    pub(crate) indices: Vec<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserMention {
    pub(crate) screen_name: String,
    pub(crate) name: String,
    pub(crate) id_str: String,
    pub(crate) indices: Vec<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtendedEntities {
    pub(crate) media: Vec<ExtendedEntitiesMedia>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtendedEntitiesMedia {
    pub(crate) id_str: String,
    pub(crate) indices: Vec<i64>,
    pub(crate) media_url: String,
    pub(crate) media_url_https: String,
    pub(crate) url: String,
    pub(crate) display_url: String,
    pub(crate) expanded_url: String,
    #[serde(rename = "type")]
    pub(crate) media_type: Type,
    pub(crate) original_info: OriginalInfo,
    pub(crate) sizes: Sizes,
    pub(crate) media_key: Option<String>,
    pub(crate) ext: Option<FluffyExt>,
    pub(crate) source_status_id_str: Option<String>,
    pub(crate) source_user_id_str: Option<String>,
    pub(crate) video_info: Option<FluffyVideoInfo>,
    pub(crate) additional_media_info: Option<AdditionalMediaInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AdditionalMediaInfo {
    pub(crate) monetizable: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FluffyExt {
    #[serde(rename = "mediaStats")]
    pub(crate) media_stats: FluffyMediaStats,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FluffyMediaStats {
    pub(crate) r: RUnion,
    pub(crate) ttl: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RRClass {
    pub(crate) ok: Ok,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Ok {
    #[serde(rename = "viewCount")]
    pub(crate) view_count: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FluffyVideoInfo {
    pub(crate) aspect_ratio: Vec<i64>,
    pub(crate) duration_millis: Option<i64>,
    pub(crate) variants: Vec<Variant>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QuotedStatusPermalink {
    pub(crate) url: String,
    pub(crate) expanded: String,
    pub(crate) display: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SelfThread {
    pub(crate) id_str: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub(crate) id_str: String,
    pub(crate) name: String,
    pub(crate) screen_name: String,
    // pub(crate) location: String,
    // pub(crate) description: String,
    // pub(crate) url: Option<String>,
    // pub(crate) entities: UserEntities,
    // pub(crate) followers_count: i64,
    // pub(crate) fast_followers_count: i64,
    // pub(crate) normal_followers_count: i64,
    // pub(crate) friends_count: i64,
    // pub(crate) listed_count: i64,
    // pub(crate) created_at: String,
    // pub(crate) favourites_count: i64,
    // pub(crate) geo_enabled: Option<bool>,
    // pub(crate) statuses_count: i64,
    // pub(crate) media_count: i64,
    pub(crate) profile_image_url_https: Option<String>,
    // pub(crate) profile_banner_url: Option<String>,
    // pub(crate) profile_image_extensions: ProfileExtensions,
    // pub(crate) profile_banner_extensions: Option<ProfileExtensions>,
    // pub(crate) profile_link_color: String,
    // pub(crate) pinned_tweet_ids: Vec<f64>,
    // pub(crate) pinned_tweet_ids_str: Vec<String>,
    // pub(crate) has_custom_timelines: Option<bool>,
    // pub(crate) profile_interstitial_type: String,
    // pub(crate) has_extended_profile: Option<bool>,
    // pub(crate) default_profile: Option<bool>,
    // pub(crate) verified: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserEntities {
    pub(crate) url: Option<PurpleUrl>,
    pub(crate) description: Description,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Description {
    pub(crate) urls: Option<Vec<UrlElement>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PurpleUrl {
    pub(crate) urls: Vec<UrlElement>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProfileExtensions {
    #[serde(rename = "mediaStats")]
    pub(crate) media_stats: ProfileImageExtensionsMediaStats,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProfileImageExtensionsMediaStats {
    pub(crate) r: MediaStatsRClass,
    pub(crate) ttl: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MediaStatsRClass {
    pub(crate) missing: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Timeline {
    pub(crate) id: String,
    pub(crate) instructions: Vec<HashMap<String, Entries>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Entries {
    #[serde(rename = "addEntries")]
    AddEntries {
        entries: Vec<Entry>,
    },
    Other(serde_json::Value),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddEntries {
    pub(crate) entries: Vec<Entry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
    #[serde(rename = "entryId")]
    pub(crate) entry_id: String,
    #[serde(rename = "sortIndex")]
    pub(crate) sort_index: String,
    pub(crate) content: EntryContent,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EntryContent {
    pub(crate) item: Option<ContentItem>,
    #[serde(rename = "timelineModule")]
    pub(crate) timeline_module: Option<TimelineModule>,
    pub(crate) operation: Option<Operation>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContentItem {
    pub(crate) content: PurpleContent,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PurpleContent {
    pub(crate) tweet: ContentTweet,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContentTweet {
    pub(crate) id: String,
    #[serde(rename = "displayType")]
    pub(crate) display_type: DisplayType,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Operation {
    pub(crate) cursor: Cursor,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Cursor {
    pub(crate) value: String,
    #[serde(rename = "cursorType")]
    pub(crate) cursor_type: String,
    #[serde(rename = "stopOnEmptyResponse")]
    pub(crate) stop_on_empty_response: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TimelineModule {
    pub(crate) items: Vec<ItemElement>,
    #[serde(rename = "displayType")]
    pub(crate) display_type: String,
    pub(crate) header: Header,
    #[serde(rename = "clientEventInfo")]
    pub(crate) client_event_info: TimelineModuleClientEventInfo,
    pub(crate) metadata: Metadata,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TimelineModuleClientEventInfo {
    pub(crate) component: Component,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Header {
    pub(crate) text: String,
    pub(crate) sticky: bool,
    #[serde(rename = "socialContext")]
    pub(crate) social_context: SocialContext,
    #[serde(rename = "displayType")]
    pub(crate) display_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SocialContext {
    #[serde(rename = "generalContext")]
    pub(crate) general_context: GeneralContext,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeneralContext {
    #[serde(rename = "contextType")]
    pub(crate) context_type: String,
    pub(crate) text: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ItemElement {
    #[serde(rename = "entryId")]
    pub(crate) entry_id: String,
    pub(crate) item: ItemItem,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ItemItem {
    pub(crate) content: FluffyContent,
    #[serde(rename = "clientEventInfo")]
    pub(crate) client_event_info: ItemClientEventInfo,
    #[serde(rename = "feedbackInfo")]
    pub(crate) feedback_info: FeedbackInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ItemClientEventInfo {
    pub(crate) component: Component,
    pub(crate) element: Element,
    pub(crate) details: Details,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Details {
    #[serde(rename = "timelinesDetails")]
    pub(crate) timelines_details: TimelinesDetails,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TimelinesDetails {
    #[serde(rename = "controllerData")]
    pub(crate) controller_data: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FluffyContent {
    pub(crate) topic: ContentTopic,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContentTopic {
    #[serde(rename = "topicId")]
    pub(crate) topic_id: String,
    #[serde(rename = "topicFunctionalityType")]
    pub(crate) topic_functionality_type: TopicFunctionalityType,
    #[serde(rename = "topicDisplayType")]
    pub(crate) topic_display_type: TopicDisplayType,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FeedbackInfo {
    #[serde(rename = "feedbackKeys")]
    pub(crate) feedback_keys: Vec<String>,
    #[serde(rename = "feedbackMetadata")]
    pub(crate) feedback_metadata: FeedbackMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Metadata {
    #[serde(rename = "gridCarouselMetadata")]
    pub(crate) grid_carousel_metadata: GridCarouselMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GridCarouselMetadata {
    #[serde(rename = "numRows")]
    pub(crate) num_rows: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RichBehavior {
    #[serde(rename = "markNotInterestedTopic")]
    pub(crate) mark_not_interested_topic: MarkNotInterestedTopic,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MarkNotInterestedTopic {
    #[serde(rename = "topicId")]
    pub(crate) topic_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RUnion {
    Enum(REnum),
    RrClass(RRClass),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum REnum {
    Missing,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Type {
    #[serde(rename = "animated_gif")]
    AnimatedGif,
    #[serde(rename = "photo")]
    Photo,
    #[serde(rename = "video")]
    Video,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Resize {
    #[serde(rename = "crop")]
    Crop,
    #[serde(rename = "fit")]
    Fit,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ContentType {
    #[serde(rename = "application/x-mpegURL")]
    ApplicationXMpegUrl,
    #[serde(rename = "video/mp4")]
    VideoMp4,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum AdvertiserAccountServiceLevel {
    #[serde(rename = "analytics")]
    Analytics,
    #[serde(rename = "media_studio")]
    MediaStudio,
    #[serde(rename = "mms")]
    Mms,
    #[serde(rename = "smb")]
    Smb,
    #[serde(rename = "subscription")]
    Subscription,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum AdvertiserAccountType {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "promotable_user")]
    PromotableUser,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TranslatorType {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "regular")]
    Regular,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DisplayType {
    Tweet,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Component {
    #[serde(rename = "suggest_topics_module")]
    SuggestTopicsModule,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Element {
    #[serde(rename = "topic")]
    Topic,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TopicDisplayType {
    Pill,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TopicFunctionalityType {
    Recommendation,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum FeedbackMetadata {
    #[serde(rename = "FcQBOQwA")]
    FcQboQwA,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum EncodedFeedbackRequest {
    #[serde(rename = "LBUeHBXEATkMAAAA")]
    LbUeHbxeaTkMaaaa,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum FeedbackType {
    RichBehavior,
}
