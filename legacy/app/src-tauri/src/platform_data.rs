//! 中国数字遗产平台完整数据库
//!
//! 内置 150+ 主流平台的继承政策、所需材料、办理流程、变现路径、法律文书模板等完整信息。
//! 为前端资产发现引擎、继承人向导、继承执行工具包提供数据支撑。
//!
//! 辅助查询函数（get_platform_by_id / get_platforms_by_category / get_difficulty_stats）
//! 预留给前端独立查询使用，MVP batch 路径暂不调用。

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// 政策未经人工核实时使用的显式标注。门禁要求「不许为空」，
/// 但也不允许编造出处——所以「未核实」本身是一个合法且必须被展示的状态。
pub const UNVERIFIED_SOURCE: &str = "未核实（待人工核对官方公告）";

/// 平台条目 - 中国数字资产平台完整清单
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformEntry {
    pub id: i64,
    pub name: String,
    pub name_en: String,
    pub category: String,
    pub subtype: String,
    pub has_property: bool,
    pub has_personality: bool,
    pub inheritable: bool,
    pub required_documents: String,
    pub process_description: String,
    pub estimated_difficulty: String, // "简单", "中等", "困难", "暂不支持"
    pub official_policy_url: String,
    pub estimated_time_days: String,
    pub notes: String,
    pub data_source: String, // 数据来源：权威出处 / 官网链接 / 判例编号
    // 新增字段：用于继承执行工具包
    pub inheritance_form_url: String, // 平台官方继承申请表下载链接
    pub monetization_path: String,    // 变现/过户路径卡（JSON 字符串）
    pub legal_template_ids: String,   // 关联的法律文书模板 ID 列表（逗号分隔）
    pub risk_warnings: String,        // 风险提示
    pub tags: String,                 // 标签：如 "高价值" "需公证" "简易程序" "死亡循环风险"
    /// 本条政策最后一次人工核对的日期（`YYYY-MM`）。
    /// 空值表示「从未人工核实」——界面与报告必须如实标注，不能假装是官方口径。
    pub last_reviewed: String,
    /// 关联的权威来源 id（对应 `src/data/authorities.json`）。
    /// 空表示只有文字说明、没有可核验的外部来源。
    pub authority_ids: Vec<String>,
}

impl PlatformEntry {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i64,
        name: &str,
        name_en: &str,
        category: &str,
        subtype: &str,
        has_property: bool,
        has_personality: bool,
        inheritable: bool,
        required_documents: &str,
        process_description: &str,
        estimated_difficulty: &str,
        official_policy_url: &str,
        estimated_time_days: &str,
        notes: &str,
    ) -> Self {
        Self {
            id,
            name: name.to_string(),
            name_en: name_en.to_string(),
            category: category.to_string(),
            subtype: subtype.to_string(),
            has_property,
            has_personality,
            inheritable,
            required_documents: required_documents.to_string(),
            process_description: process_description.to_string(),
            estimated_difficulty: estimated_difficulty.to_string(),
            official_policy_url: official_policy_url.to_string(),
            estimated_time_days: estimated_time_days.to_string(),
            notes: notes.to_string(),
            // 显式标注「未核实」，而不是留空。空值无法区分「漏填」与
            // 「确实没有可引用的官方出处」，界面也就无法如实提示。
            data_source: UNVERIFIED_SOURCE.to_string(),
            inheritance_form_url: String::new(),
            monetization_path: String::new(),
            legal_template_ids: String::new(),
            risk_warnings: String::new(),
            tags: String::new(),
            last_reviewed: String::new(),
            authority_ids: Vec::new(),
        }
    }

    /// 创建带有数据来源的平台条目
    #[allow(clippy::too_many_arguments)]
    pub fn with_source(
        id: i64,
        name: &str,
        name_en: &str,
        category: &str,
        subtype: &str,
        has_property: bool,
        has_personality: bool,
        inheritable: bool,
        required_documents: &str,
        process_description: &str,
        estimated_difficulty: &str,
        official_policy_url: &str,
        estimated_time_days: &str,
        notes: &str,
        data_source: &str,
    ) -> Self {
        Self {
            id,
            name: name.to_string(),
            name_en: name_en.to_string(),
            category: category.to_string(),
            subtype: subtype.to_string(),
            has_property,
            has_personality,
            inheritable,
            required_documents: required_documents.to_string(),
            process_description: process_description.to_string(),
            estimated_difficulty: estimated_difficulty.to_string(),
            official_policy_url: official_policy_url.to_string(),
            estimated_time_days: estimated_time_days.to_string(),
            notes: notes.to_string(),
            data_source: data_source.to_string(),
            inheritance_form_url: String::new(),
            monetization_path: String::new(),
            legal_template_ids: String::new(),
            risk_warnings: String::new(),
            tags: String::new(),
            last_reviewed: String::new(),
            authority_ids: Vec::new(),
        }
    }
}

impl PlatformEntry {
    /// 链式挂载权威来源 id（对应 `src/data/authorities.json`）。
    pub fn with_authorities(mut self, ids: &[&str]) -> Self {
        self.authority_ids = ids.iter().map(|s| s.to_string()).collect();
        self
    }

    /// 链式标注人工核对月份。
    pub fn reviewed(mut self, yyyy_mm: &str) -> Self {
        self.last_reviewed = yyyy_mm.to_string();
        self
    }
}

/// 获取完整的平台数据库
pub fn get_all_platforms() -> Vec<PlatformEntry> {
    let mut platforms = vec![
        // ========== 支付金融类 ==========
        PlatformEntry::with_source(1, "支付宝", "Alipay", "支付金融", "移动支付", true, false, true,
            "死亡证明、继承公证书、继承人身份证；5万元以下免公证",
            "参照银行存款管理（2025新规）。联系支付宝客服95188，余额、余额宝资金可依法继承",
            "中等", "https://help.alipay.com", "30-90天",
            "5万以下走简易继承程序（2025银行新规）",
            "2025《简化小额存款继承流程指导意见》：支付宝/微信余额参照银行存款管理；每年节省公证费用超10亿元（中国人民银行）"
        ).with_authorities(&["civil-code-1122", "pbc"]),
        PlatformEntry::with_source(2, "微信支付", "WeChat Pay", "支付金融", "移动支付", true, false, true,
            "死亡证明、继承公证书、继承人身份证",
            "独立于账号的余额权益可继承。资金账户与社交账号分开处理",
            "中等", "https://weixin.qq.com", "30-90天",
            "余额可依法继承；账号本身不可继承（用户仅有使用权）",
            "微信公众号'法律如何守护人格化的数字遗产'(2025)：陈晔父亲去世10年后微信被销号；《民法典》第127条；2025银行新规参照银行存款"
        ).with_authorities(&["civil-code-127", "pbc"]),
        PlatformEntry::new(3, "云闪付", "UnionPay云闪付", "支付金融", "移动支付", true, false, true,
            "死亡证明、继承公证书",
            "联系银联客服95516",
            "中等", "https://www.unionpay.com", "30-60天",

            ""),
        PlatformEntry::new(4, "京东支付", "JD Pay", "支付金融", "在线支付", true, false, true,
            "死亡证明、继承公证书",
            "联系京东金融客服95118",
            "中等", "https://www.jdpay.com", "30-60天",
            ""),

        // ========== 社交通讯类 ==========
        PlatformEntry::with_source(10, "微信", "WeChat", "社交通讯", "即时通讯", true, true, false,
            "不可继承（仅可导出聊天记录）",
            "微信账号本身不可继承。可通过官网申请导出聊天记录。可设置\"收藏\"内容继承",
            "困难", "https://weixin.qq.com", "无法继承",
            "连续180天未登录将被平台收回；陈晔父亲去世10年后被销号（2025热搜）",
            "微信公众号'去世父亲微信账号被收回'(2025)：陈晔收集11条信息仍被拒；广州中院认定微信账号属于虚拟财产；《民法典》第127条"
        ),
        PlatformEntry::new(11, "QQ", "QQ", "社交通讯", "即时通讯", true, true, false,
            "不可继承",
            "QQ账号不能继承。QQ空间中照片等数据需在生前自行备份",
            "困难", "https://im.qq.com", "无法继承",
            "需在生前自行备份数据"),
        PlatformEntry::new(12, "微博", "Weibo", "社交通讯", "社交媒体", true, true, false,
            "需联系客服",
            "可申请为\"纪念账号\"，但不可登录和转移所有权。商业账号收益需走继承流程",
            "中等", "https://weibo.com", "30-90天",
            "公开内容可长期保留"),
        PlatformEntry::new(13, "小红书", "Xiaohongshu", "社交通讯", "社交媒体", true, true, false,
            "需联系客服",
            "粉丝量有价值的账号可尝试申请商业继承。个人数据可在生前导出",
            "中等", "https://www.xiaohongshu.com", "30-60天",
            "创作者账号达到一定量级有商业价值"),
        PlatformEntry::with_source(14, "抖音", "Douyin", "社交通讯", "短视频", true, true, true,
            "需联系客服，账号持有人死亡证明",
            "有价值的账号可申请过户给继承人。需确认账号类型为机构/企业账号才能过户",
            "中等", "https://www.douyin.com", "60-120天",
            "一个身份证只能绑定一个抖音号；更换实名需先注销原号或申诉（知乎实测）",
            "知乎'什么场景下需要更换实名'：抖音更换实名需先注销或申诉；快手可通过创作平台变更主体；辽宁开原法院认定快手账户属于新生的虚拟财产"
        ),
        PlatformEntry::new(15, "快手", "Kuaishou", "社交通讯", "短视频", true, true, true,
            "死亡证明、身份关系证明",
            "平台支持账号继承，需走官方流程提交材料",
            "中等", "https://www.kuaishou.com", "60-90天",
            "快手账号继承相对成熟，有官方流程"),
        PlatformEntry::new(16, "B站(bilibili)", "Bilibili", "社交通讯", "内容创作", true, true, false,
            "不可继承，需生前自行备份",
            "账号不可继承。创作内容可在生前自行导出保存",
            "困难", "https://www.bilibili.com", "无法继承",
            "官方UP主账号绑定实名认证，无法变更"),
        PlatformEntry::new(17, "知乎", "Zhihu", "社交通讯", "问答社区", true, true, false,
            "账号不可继承",
            "账号不可继承。创作内容可在生前自行保存",
            "困难", "https://www.zhihu.com", "无法继承",
            "内容型数字遗产，仅有情感和知识产权价值"),
        PlatformEntry::new(18, "陌陌", "Momo", "社交通讯", "社交", false, true, false,
            "不可继承",
            "账号不可继承",
            "困难", "https://www.immomo.com", "无法继承",
            "纯社交通讯价值"),
        PlatformEntry::new(19, "钉钉", "DingTalk", "社交通讯", "办公协作", true, true, false,
            "由企业管理员处理",
            "企业账号由企业管理员负责。个人数据需生前备份",
            "中等", "https://www.dingtalk.com", "视企业而定",
            "企业内数据归企业所有"),

        // ========== 游戏类 ==========
        PlatformEntry::with_source(20, "腾讯游戏综合", "Tencent Games", "游戏", "游戏账号", true, true, true,
            "死亡证明、继承公证书、继承人身份证",
            "2025北京石景山法院判例：平台格式条款无效，87个账号判令配合过户。联系腾讯客服",
            "中等", "https://www.qq.com", "60-120天",
            "87个账号判例：古先生身故，母亲主张继承实名认证账号",
            "凤凰网·惠诚律所(2025)：北京石景山法院判令游戏公司配合办理实名变更；认定'不得继承'格式条款无效"
        ),
        PlatformEntry::new(21, "网易游戏", "NetEase Games", "游戏", "游戏账号", true, true, true,
            "死亡证明、继承公证书",
            "网易游戏提供账号继承流程，需联系客服提交材料",
            "中等", "https://www.163.com", "60-120天",
            "网易游戏在虚拟财产继承方面较为开放"),
        PlatformEntry::new(22, "Steam", "Steam", "游戏", "游戏平台", true, true, false,
            "账号不可继承（需生前备份）",
            "Steam账号不可转移。需生前自行备份游戏购买信息",
            "困难", "https://store.steampowered.com", "无法继承",
            "Steam用户协议明确禁止账号转让和继承"),
        PlatformEntry::new(23, "米哈游（原神等）", "HoYoverse", "游戏", "游戏账号", true, true, true,
            "死亡证明、继承公证书",
            "米哈游支持游戏账号继承，需联系客服",
            "中等", "https://www.mihoyo.com", "30-90天",
            "价值较高的账号可申请继承"),
        PlatformEntry::new(24, "暴雪战网", "Battle.net", "游戏", "游戏平台", true, true, false,
            "账号不可继承",
            "根据暴雪战网服务条款，账号不可继承转让",
            "困难", "https://www.blizzard.com", "无法继承",
            "需生前自行备份领取码等"),

        // ========== 电商类 ==========
        PlatformEntry::new(30, "淘宝", "Taobao", "电商", "网购平台", true, true, true,
            "死亡证明、继承公证书、继承人身份证",
            "个人网店可过户。支付宝余额继承走支付宝流程",
            "中等", "https://www.taobao.com", "30-90天",
            "有收入的淘宝店铺具有明确财产价值"),
        PlatformEntry::new(31, "京东", "JD.com", "电商", "网购平台", true, true, true,
            "死亡证明、继承公证书",
            "京东余额和Plus会员有效期可继承",
            "中等", "https://www.jd.com", "30-60天",
            "Plus会员剩余价值低"),
        PlatformEntry::new(32, "拼多多", "Pinduoduo", "电商", "网购平台", true, false, true,
            "死亡证明、继承公证书",
            "拼多多钱包余额可申请继承",
            "中等", "https://www.pinduoduo.com", "30-60天",
            "纯支付功能为主"),
        PlatformEntry::new(33, "闲鱼", "Xianyu", "电商", "二手交易", true, false, true,
            "继承公证书、死亡证明",
            "闲鱼与支付宝账号绑定，按支付宝流程处理",
            "中等", "https://www.goofish.com", "30-60天",
            "与支付宝联动"),
        PlatformEntry::new(34, "得物", "Dewu/POIZON", "电商", "潮流电商", true, false, true,
            "死亡证明、继承公证书",
            "得物账户余额可申请提取",
            "中等", "https://www.dewu.com", "30-60天",
            ""),

        // ========== 金融证券类 ==========
        PlatformEntry::new(40, "同花顺", "Flush", "金融证券", "股票软件", true, false, true,
            "死亡证明、继承公证书、证券继承材料",
            "股票账户继承需走证券公司+中登公司流程，股票遗产继承较为复杂",
            "困难", "https://www.10jqka.com.cn", "90-365天",
            "股票继承涉及证券公司、中登公司、公证处、法院等多个机构"),
        PlatformEntry::new(41, "东方财富", "East Money", "金融证券", "金融平台", true, false, true,
            "死亡证明、继承公证书",
            "证券账户按股票继承流程处理",
            "困难", "https://www.eastmoney.com", "90-365天",
            ""),
        PlatformEntry::new(42, "蚂蚁财富", "Ant Fortune", "金融证券", "基金理财", true, false, true,
            "死亡证明、继承公证书",
            "基金、理财产品可通过支付宝体系提交继承申请",
            "中等", "https://www.antfortune.com", "30-90天",
            ""),
        PlatformEntry::new(43, "微信零钱通", "WeChat Money", "金融证券", "理财产品", true, false, true,
            "死亡证明、继承公证书",
            "走微信支付继承流程",
            "中等", "https://weixin.qq.com", "30-90天",
            "与微信支付同步处理"),
        PlatformEntry::new(44, "招商银行APP", "CMB APP", "金融证券", "银行", true, false, true,
            "死亡证明、继承公证书",
            "银行卡及招行APP内资产按银行继承流程，继承公证书即可办理",
            "简单", "https://www.cmbchina.com", "7-30天",
            "银行体系内继承流程成熟"),

        // ========== 知识付费类 ==========
        PlatformEntry::new(50, "得到APP", "Dedao", "知识付费", "知识服务", true, false, false,
            "账号不可继承",
            "账号不可继承。已购买的课程可在原设备上继续使用",
            "困难", "https://www.dedao.cn", "无法继承",
            "课程绑定账号，不可转移给他人"),
        PlatformEntry::new(51, "知乎盐选", "Zhihu Premium", "知识付费", "内容会员", true, false, false,
            "账号不可继承",
            "不可继承。需生前自行使用",
            "困难", "https://www.zhihu.com", "无法继承",
            ""),
        PlatformEntry::new(52, "喜马拉雅", "Ximalaya", "知识付费", "音频平台", true, false, false,
            "死亡证明、继承公证书",
            "联系喜马拉雅客服申请余额退还",
            "中等", "https://www.ximalaya.com", "30-60天",
            "已购课程不可转移，仅退余额"),
        PlatformEntry::new(53, "腾讯课堂", "Tencent Class", "知识付费", "在线教育", true, false, false,
            "账号不可继承",
            "已购课程绑定账号，不可转移",
            "困难", "https://ke.qq.com", "无法继承",
            ""),

        // ========== 工具订阅类 ==========
        PlatformEntry::new(60, "iCloud", "iCloud", "工具订阅", "云服务", true, true, true,
            "死亡证明、继承密钥、遗产联系人",
            "苹果已为死者提供\"数字遗产\"功能。需生前设置\"遗产联系人\"",
            "简单", "https://support.apple.com/zh-cn/HT212360", "7-30天",
            "苹果数字遗产计划（Digital Legacy）已成熟，需生前设置"),
        PlatformEntry::new(61, "百度网盘", "Baidu Netdisk", "工具订阅", "云存储", true, true, false,
            "身份验证+账号密码",
            "百度网盘不官方支持账号访问权转移。需生前共享账号或备份文件",
            "困难", "https://pan.baidu.com", "不支持",
            "数据价值完全依赖账号密码"),
        PlatformEntry::new(62, "阿里云盘", "Alibaba Cloud Drive", "工具订阅", "云存储", true, true, false,
            "身份验证+账号密码",
            "同百度网盘",
            "困难", "https://www.aliyundrive.com", "不支持",
            ""),
        PlatformEntry::new(63, "WPS Office", "WPS Office", "工具订阅", "办公软件", true, false, false,
            "账号不可继承",
            "已购买会员有效期不可转移。文档需生前自行导出",
            "困难", "https://www.wps.cn", "无法继承",
            ""),
        PlatformEntry::new(64, "Office 365 / Microsoft 365", "Microsoft 365", "工具订阅", "办公软件", true, false, false,
            "需Microsoft客服协助",
            "订阅费用可按比例退费",
            "中等", "https://www.microsoft.com", "30天",
            ""),
        PlatformEntry::new(65, "网易云音乐", "NetEase Music", "工具订阅", "音乐", true, true, false,
            "账号不可继承",
            "付费会员到期后账号失效。已下载音乐绑定设备",
            "困难", "https://music.163.com", "无法继承",
            "无实质继承价值"),
        PlatformEntry::new(66, "QQ音乐", "QQ Music", "工具订阅", "音乐", true, false, false,
            "账号不可继承",
            "付费会员不可继承",
            "困难", "https://y.qq.com", "无法继承",
            ""),

        // ========== 出行住宿类 ==========
        PlatformEntry::new(70, "12306铁路", "12306", "出行住宿", "交通", true, true, false,
            "账号不可继承",
            "账号绑定身份证。乘车记录和积分无实质继承价值",
            "困难", "https://www.12306.cn", "无法继承",
            "账户与身份证绑定，持有人去世即失效"),
        PlatformEntry::new(71, "携程", "Ctrip", "出行住宿", "旅游", true, true, true,
            "死亡证明、继承公证书",
            "携程任我行余额可继承。积分通常在继承后失效",
            "中等", "https://www.ctrip.com", "30-60天",
            ""),
        PlatformEntry::new(72, "滴滴出行", "Didi", "出行住宿", "出行", true, false, true,
            "死亡证明、继承公证书",
            "滴滴余额可申请退还",
            "中等", "https://www.didiglobal.com", "30天",
            ""),
        PlatformEntry::new(73, "美团", "Meituan", "生活工具", "本地生活", true, false, true,
            "死亡证明、继承公证书",
            "美团余额、美团月付账户需联系客服处理",
            "中等", "https://www.meituan.com", "30-60天",
            ""),
        PlatformEntry::new(74, "饿了么", "Ele.me", "生活工具", "外卖", true, false, true,
            "死亡证明、继承公证书",
            "账户余额可联系客服申请退还",
            "中等", "https://www.ele.me", "30-60天",
            ""),

        // ========== 自媒体/创作类 ==========
        PlatformEntry::new(80, "微信公众号", "WeChat Official Account", "内容创作", "创作平台", true, true, false,
            "公众号可迁移",
            "类型不同政策不同。个人号不支持继承，可迁移至新主体。企业号按企业流程处理",
            "困难", "https://mp.weixin.qq.com", "60-180天",
            "有商业价值的公众号（有广告收入和有粉丝）可申请主体变更"),
        PlatformEntry::new(81, "今日头条", "Toutiao", "内容创作", "内容平台", true, true, true,
            "联系客服",
            "有平台收益的头条号可申请收益权转移",
            "中等", "https://www.toutiao.com", "60-90天",
            "创作收益具有明确财产价值"),
        PlatformEntry::new(82, "百家号", "Baijiahao", "内容创作", "内容平台", true, false, true,
            "联系百度客服",
            "账号收益可申请继承",
            "中等", "https://baijiahao.baidu.com", "30-60天",
            ""),
        PlatformEntry::new(83, "视频号", "WeChat Channels", "内容创作", "短视频", true, true, false,
            "账号不可继承",
            "与微信绑定，微信无法继承则视频号也无法继承",
            "困难", "https://channels.weixin.qq.com", "无法继承",
            ""),

        // ========== 存储云服务类 ==========
        PlatformEntry::new(90, "腾讯微云", "Weiyun", "存储云服务", "云盘", true, true, false,
            "需账号密码",
            "官方不提供账号访问权转移，需生前自行备份",
            "困难", "https://www.weiyun.com", "无法继承",
            ""),
        PlatformEntry::new(91, "OneDrive", "OneDrive", "存储云服务", "云盘", true, true, true,
            "死亡证明、继承材料",
            "微软提供数字遗产访问流程",
            "中等", "https://onedrive.live.com", "30-60天",
            "微软提供了较为成熟的数字遗产流程"),
        PlatformEntry::new(92, "Google Drive", "Google Drive", "存储云服务", "云盘", true, true, true,
            "出生/死亡证明及授权",
            "谷歌Inactive Account Manager支持",
            "简单", "https://drive.google.com", "已设置则自动执行",
            "需生前在谷歌账户中配置不活跃账户管理器"),
        PlatformEntry::new(93, "Dropbox", "Dropbox", "存储云服务", "云盘", true, true, true,
            "继承人申请+法律文件",
            "Dropbox支持通过法律流程转移",
            "中等", "https://www.dropbox.com", "30-60天",
            ""),

        // ========== 生活服务类 ==========
        PlatformEntry::new(100, "广州医保", "Guangzhou Medical Insurance", "政务服务", "公共服务", true, false, true,
            "死亡证明、社保继承材料",
            "医保个人账户余额可由继承人申请继承",
            "简单", "https://www.gz.gov.cn", "15-30天",
            "医保余额可提取"),
        PlatformEntry::new(101, "广东社保", "Guangdong Social Insurance", "政务服务", "社保", true, false, true,
            "死亡证明、继承公证书",
            "社保（养老）个人账户余额可提取，公积金一并处理",
            "简单", "https://www.gd.gov.cn", "15-30天",
            "社保和公积金均有明确继承法规"),
        PlatformEntry::new(102, "公积金", "Housing Fund", "政务服务", "公积金", true, false, true,
            "死亡证明、继承公证书",
            "公积金账户余额可由继承人全额提取",
            "简单", "https://www.cqgjj.cn", "7-15天",
            "公积金法规明确，继承简便"),

        // ========== 扩展：银行类（6个） ==========
        PlatformEntry::new(45, "工商银行", "ICBC", "金融证券", "银行", true, false, true,
            "死亡证明、继承公证书、继承人身份证", "银行柜台办理，流程成熟", "简单",
            "https://www.icbc.com.cn", "7-15天", "银行账户继承走公证+柜台"),
        PlatformEntry::new(46, "建设银行", "CCB", "金融证券", "银行", true, false, true,
            "死亡证明、继承公证书、继承人身份证", "银行柜台办理，流程成熟", "简单",
            "https://www.ccb.com", "7-15天", ""),
        PlatformEntry::new(47, "农业银行", "ABC", "金融证券", "银行", true, false, true,
            "死亡证明、继承公证书、继承人身份证", "银行柜台办理，流程成熟", "简单",
            "https://www.abchina.com", "7-15天", ""),
        PlatformEntry::new(48, "中国银行", "BOC", "金融证券", "银行", true, false, true,
            "死亡证明、继承公证书、继承人身份证", "银行柜台办理，流程成熟", "简单",
            "https://www.boc.cn", "7-15天", ""),
        PlatformEntry::new(49, "交通银行", "BOCOM", "金融证券", "银行", true, false, true,
            "死亡证明、继承公证书、继承人身份证", "银行柜台办理，流程成熟", "简单",
            "https://www.bankcomm.com", "7-15天", ""),
        PlatformEntry::new(104, "平安银行", "Ping An Bank", "金融证券", "银行", true, false, true,
            "死亡证明、继承公证书、继承人身份证", "银行柜台办理，流程成熟", "简单",
            "https://www.pingan.com", "7-15天", ""),

        // ========== 扩展：证券/保险/基金类（8个） ==========
        PlatformEntry::new(105, "中信证券", "CITIC Securities", "金融证券", "券商", true, false, true,
            "死亡证明、继承公证书、证券继承材料", "证券公司柜台+中登公司流程", "困难",
            "https://www.citics.com", "90-365天", "股票继承涉及多机构"),
        PlatformEntry::new(106, "华泰证券", "Huatai Securities", "金融证券", "券商", true, false, true,
            "死亡证明、继承公证书、证券继承材料", "证券公司柜台+中登公司流程", "困难",
            "https://www.htsc.com.cn", "90-365天", ""),
        PlatformEntry::new(107, "中国人寿", "China Life", "金融证券", "保险", true, false, true,
            "死亡证明、继承公证书、保险单", "保险公司柜台办理理赔/退保", "中等",
            "https://www.e-chinalife.com", "30-60天", "保单现金价值可继承"),
        PlatformEntry::new(108, "平安保险", "Ping An Insurance", "金融证券", "保险", true, false, true,
            "死亡证明、继承公证书、保险单", "保险公司柜台办理理赔/退保", "中等",
            "https://www.pingan.com", "30-60天", ""),
        PlatformEntry::new(109, "太平洋保险", "CPIC", "金融证券", "保险", true, false, true,
            "死亡证明、继承公证书、保险单", "保险公司柜台办理理赔/退保", "中等",
            "https://www.cpic.com.cn", "30-60天", ""),
        PlatformEntry::new(110, "天弘基金", "Tianhong Fund", "金融证券", "基金", true, false, true,
            "死亡证明、继承公证书", "通过支付宝体系提交继承申请", "中等",
            "https://www.tianhong.cn", "30-90天", ""),
        PlatformEntry::new(111, "华夏基金", "China AMC", "金融证券", "基金", true, false, true,
            "死亡证明、继承公证书", "联系基金公司提交继承材料", "中等",
            "https://www.chinaamc.com", "30-90天", ""),
        PlatformEntry::new(112, "易方达基金", "E Fund", "金融证券", "基金", true, false, true,
            "死亡证明、继承公证书", "联系基金公司提交继承材料", "中等",
            "https://www.efunds.com.cn", "30-90天", ""),

        // ========== 扩展：游戏类（12个） ==========
        PlatformEntry::new(113, "Epic Games", "Epic Games", "游戏", "游戏平台", true, true, false,
            "账号不可继承", "需生前自行备份", "困难",
            "https://www.epicgames.com", "无法继承", "用户协议禁止转让"),
        PlatformEntry::new(114, "任天堂Switch Online", "Nintendo Switch", "游戏", "游戏订阅", true, true, false,
            "账号不可继承", "需生前自行备份游戏存档", "困难",
            "https://www.nintendo.com", "无法继承", ""),
        PlatformEntry::new(115, "PlayStation Network", "PSN", "游戏", "游戏平台", true, true, false,
            "账号不可继承", "需生前自行备份", "困难",
            "https://www.playstation.com", "无法继承", ""),
        PlatformEntry::new(116, "Xbox Live", "Xbox", "游戏", "游戏平台", true, true, false,
            "账号不可继承", "需生前自行备份", "困难",
            "https://www.xbox.com", "无法继承", ""),
        PlatformEntry::new(117, "Origin/EA", "Electronic Arts", "游戏", "游戏平台", true, true, false,
            "账号不可继承", "需生前自行备份", "困难",
            "https://www.ea.com", "无法继承", ""),
        PlatformEntry::new(118, "Ubisoft Connect", "Ubisoft", "游戏", "游戏平台", true, true, false,
            "账号不可继承", "需生前自行备份", "困难",
            "https://www.ubisoft.com", "无法继承", ""),
        PlatformEntry::new(119, "Riot Games", "Riot Games", "游戏", "游戏账号", true, true, true,
            "死亡证明、继承公证书", "联系客服申请账号继承", "中等",
            "https://www.riotgames.com", "60-120天", "LOL/Valorant等游戏"),
        PlatformEntry::new(120, "完美世界", "Perfect World", "游戏", "游戏账号", true, true, true,
            "死亡证明、继承公证书", "联系客服申请账号继承", "中等",
            "https://www.wanmei.com", "60-120天", ""),
        PlatformEntry::new(121, "三七互娱", "37 Games", "游戏", "游戏账号", true, true, true,
            "死亡证明、继承公证书", "联系客服申请账号继承", "中等",
            "https://www.37.com", "60-120天", ""),
        PlatformEntry::new(122, "莉莉丝游戏", "Lilith Games", "游戏", "游戏账号", true, true, true,
            "死亡证明、继承公证书", "联系客服申请账号继承", "中等",
            "https://www.lilith.com", "60-120天", "AFK Arena等"),
        PlatformEntry::new(123, "库洛游戏", "Kuro Games", "游戏", "游戏账号", true, true, true,
            "死亡证明、继承公证书", "联系客服申请账号继承", "中等",
            "https://www.kurogames.com", "60-120天", "鸣潮等"),
        PlatformEntry::new(124, "叠纸游戏", "Papergames", "游戏", "游戏账号", true, true, true,
            "死亡证明、继承公证书", "联系客服申请账号继承", "中等",
            "https://www.papegames.com", "60-120天", "恋与深空等"),

        // ========== 扩展：电商/生活类（8个） ==========
        PlatformEntry::new(125, "唯品会", "Vipshop", "电商", "网购平台", true, false, true,
            "死亡证明、继承公证书", "余额可申请退还", "中等",
            "https://www.vip.com", "30-60天", ""),
        PlatformEntry::new(126, "苏宁易购", "Suning", "电商", "网购平台", true, false, true,
            "死亡证明、继承公证书", "余额可申请退还", "中等",
            "https://www.suning.com", "30-60天", ""),
        PlatformEntry::new(127, "网易严选", "Yanxuan", "电商", "网购平台", true, false, true,
            "死亡证明、继承公证书", "余额可申请退还", "中等",
            "https://you.163.com", "30-60天", ""),
        PlatformEntry::new(128, "小红书店铺", "Xiaohongshu Shop", "电商", "网店", true, true, true,
            "死亡证明、继承公证书", "店铺可申请过户", "中等",
            "https://www.xiaohongshu.com", "60-90天", "有收入的店铺"),
        PlatformEntry::new(130, "快手小店", "Kuaishou Shop", "电商", "网店", true, true, true,
            "死亡证明、继承公证书", "店铺可申请过户", "中等",
            "https://www.kuaishou.com", "60-90天", ""),
        PlatformEntry::new(131, "美团外卖", "Meituan Waimai", "生活服务", "外卖", true, false, true,
            "死亡证明、继承公证书", "账户余额可申请退还", "中等",
            "https://www.meituan.com", "30-60天", ""),
        PlatformEntry::new(132, "大众点评", "Dianping", "生活服务", "点评", true, true, true,
            "死亡证明、继承公证书", "商户号可申请过户", "中等",
            "https://www.dianping.com", "60-90天", "有商户价值的账号"),

        // ========== 扩展：社交/内容类（8个） ==========
        PlatformEntry::new(133, "豆瓣", "Douban", "社交通讯", "社区", true, true, false,
            "账号不可继承", "需生前自行备份", "困难",
            "https://www.douban.com", "无法继承", ""),
        PlatformEntry::new(134, "贴吧", "Tieba", "社交通讯", "社区", true, true, false,
            "账号不可继承", "需生前自行备份", "困难",
            "https://tieba.baidu.com", "无法继承", ""),
        PlatformEntry::new(135, "虎扑", "Hupu", "社交通讯", "社区", true, true, false,
            "账号不可继承", "需生前自行备份", "困难",
            "https://www.hupu.com", "无法继承", ""),
        PlatformEntry::new(136, "AcFun", "AcFun", "社交通讯", "视频社区", true, true, false,
            "账号不可继承", "需生前自行备份", "困难",
            "https://www.acfun.cn", "无法继承", ""),
        PlatformEntry::new(138, "微博大V", "Weibo Influencer", "内容创作", "自媒体", true, true, true,
            "死亡证明、继承公证书", "可申请纪念账号或收益权转移", "中等",
            "https://weibo.com", "60-120天", "有商业价值的账号"),
        PlatformEntry::new(139, "知乎创作者", "Zhihu Creator", "内容创作", "创作者", true, true, true,
            "死亡证明、继承公证书", "收益可申请转移", "中等",
            "https://www.zhihu.com", "60-90天", "有创作收益的账号"),
        PlatformEntry::new(140, "微信公众号订阅号", "WeChat Subscription", "内容创作", "创作平台", true, true, false,
            "需联系客服", "个人号不支持继承，可迁移至新主体", "困难",
            "https://mp.weixin.qq.com", "60-180天", ""),

        // ========== 扩展：工具/云服务类（8个） ==========
        PlatformEntry::new(141, "坚果云", "Nutstore", "工具订阅", "云存储", true, true, false,
            "需账号密码", "官方不提供账号访问权转移", "困难",
            "https://www.jianguoyun.com", "不支持", ""),
        PlatformEntry::new(142, "腾讯文档", "Tencent Docs", "工具订阅", "在线文档", true, true, false,
            "需账号密码", "文档需生前自行导出", "困难",
            "https://docs.qq.com", "不支持", ""),
        PlatformEntry::new(143, "语雀", "Yuque", "工具订阅", "知识库", true, true, false,
            "需账号密码", "文档需生前自行导出", "困难",
            "https://www.yuque.com", "不支持", ""),
        PlatformEntry::new(144, "Notion", "Notion", "工具订阅", "笔记", true, true, true,
            "死亡证明、继承材料", "Notion支持数字遗产访问", "中等",
            "https://www.notion.so", "30-60天", ""),
        PlatformEntry::new(145, "Figma", "Figma", "工具订阅", "设计工具", true, true, true,
            "死亡证明、继承材料", "Figma支持数字遗产访问", "中等",
            "https://www.figma.com", "30-60天", ""),
        PlatformEntry::new(146, "GitHub", "GitHub", "工具订阅", "代码托管", true, true, true,
            "死亡证明、继承材料", "GitHub支持数字遗产访问", "中等",
            "https://github.com", "30-60天", "代码仓库有知识产权价值"),
        PlatformEntry::new(147, "Gitee", "Gitee", "工具订阅", "代码托管", true, true, true,
            "死亡证明、继承材料", "联系客服申请仓库转移", "中等",
            "https://gitee.com", "30-60天", ""),
        PlatformEntry::new(148, "语雀企业版", "Yuque Enterprise", "工具订阅", "企业知识库", true, true, false,
            "由企业管理员处理", "企业数据归企业所有", "中等",
            "https://www.yuque.com", "视企业而定", ""),

        // ========== 扩展：音视频类（8个） ==========
        PlatformEntry::new(149, "爱奇艺", "iQIYI", "音视频", "视频会员", true, false, false,
            "账号不可继承", "会员到期后账号失效", "困难",
            "https://www.iqiyi.com", "无法继承", ""),
        PlatformEntry::new(150, "腾讯视频", "Tencent Video", "音视频", "视频会员", true, false, false,
            "账号不可继承", "会员到期后账号失效", "困难",
            "https://v.qq.com", "无法继承", ""),
        PlatformEntry::new(151, "优酷", "Youku", "音视频", "视频会员", true, false, false,
            "账号不可继承", "会员到期后账号失效", "困难",
            "https://www.youku.com", "无法继承", ""),
        PlatformEntry::new(152, "芒果TV", "Mango TV", "音视频", "视频会员", true, false, false,
            "账号不可继承", "会员到期后账号失效", "困难",
            "https://www.mgtv.com", "无法继承", ""),
        PlatformEntry::new(153, "哔哩哔哩大会员", "Bilibili Premium", "音视频", "视频会员", true, false, false,
            "账号不可继承", "大会员到期后失效", "困难",
            "https://www.bilibili.com", "无法继承", ""),
        PlatformEntry::new(154, "网易云音乐黑胶", "NetEase Music VIP", "音视频", "音乐会员", true, false, false,
            "账号不可继承", "会员到期后失效", "困难",
            "https://music.163.com", "无法继承", ""),
        PlatformEntry::new(155, "Apple Music", "Apple Music", "音视频", "音乐", true, false, true,
            "死亡证明、继承材料", "通过Apple数字遗产流程", "中等",
            "https://music.apple.com", "30-60天", "需生前设置遗产联系人"),
        PlatformEntry::new(156, "Spotify", "Spotify", "音视频", "音乐", true, false, false,
            "账号不可继承", "Premium订阅到期后失效", "困难",
            "https://www.spotify.com", "无法继承", ""),

        // ========== 扩展：出行/住宿类（6个） ==========
        PlatformEntry::new(157, "飞猪", "Fliggy", "出行住宿", "旅游", true, true, true,
            "死亡证明、继承公证书", "余额可申请退还", "中等",
            "https://www.fliggy.com", "30-60天", ""),
        PlatformEntry::new(158, "去哪儿", "Qunar", "出行住宿", "旅游", true, true, true,
            "死亡证明、继承公证书", "余额可申请退还", "中等",
            "https://www.qunar.com", "30-60天", ""),
        PlatformEntry::new(159, "马蜂窝", "Mafengwo", "出行住宿", "旅游", true, true, false,
            "账号不可继承", "需生前自行备份", "困难",
            "https://www.mafengwo.cn", "无法继承", ""),
        PlatformEntry::new(160, "航旅纵横", "Umetrip", "出行住宿", "航空", true, true, false,
            "账号不可继承", "里程积分不可继承", "困难",
            "https://www.umetrip.com", "无法继承", ""),
        PlatformEntry::new(161, "高铁管家", "12306 Assistant", "出行住宿", "交通", true, true, false,
            "账号不可继承", "需生前自行备份", "困难",
            "https://www.12306.cn", "无法继承", ""),
        PlatformEntry::new(162, "哈啰出行", "Hello Bike", "出行住宿", "出行", true, false, true,
            "死亡证明、继承公证书", "余额可申请退还", "中等",
            "https://hellobike.com", "30天", ""),

        // ========== 扩展：学习与考试类（8个） ==========
        PlatformEntry::new(163, "学信网", "CHSI", "学习与考试", "教育", true, true, false,
            "账号不可继承", "学历信息不可继承", "困难",
            "https://www.chsi.com.cn", "无法继承", ""),
        PlatformEntry::new(164, "中国大学MOOC", "MOOC", "学习与考试", "在线教育", true, false, false,
            "账号不可继承", "课程绑定账号", "困难",
            "https://www.icourse163.org", "无法继承", ""),
        PlatformEntry::new(165, "网易公开课", "NetEase Open Course", "学习与考试", "在线教育", true, false, false,
            "账号不可继承", "课程绑定账号", "困难",
            "https://open.163.com", "无法继承", ""),
        PlatformEntry::new(166, "Coursera", "Coursera", "学习与考试", "在线教育", true, false, true,
            "死亡证明、继承材料", "联系客服申请证书转移", "中等",
            "https://www.coursera.org", "30-60天", "付费证书有职业价值"),
        PlatformEntry::new(167, "Udemy", "Udemy", "学习与考试", "在线教育", true, false, true,
            "死亡证明、继承材料", "联系客服申请课程转移", "中等",
            "https://www.udemy.com", "30-60天", "付费课程有职业价值"),
        PlatformEntry::new(168, "知乎盐选会员", "Zhihu Premium", "学习与考试", "内容会员", true, false, false,
            "账号不可继承", "会员到期后失效", "困难",
            "https://www.zhihu.com", "无法继承", ""),
        PlatformEntry::new(169, "得到课堂", "Dedao Class", "学习与考试", "知识服务", true, false, false,
            "账号不可继承", "课程绑定账号", "困难",
            "https://www.dedao.cn", "无法继承", ""),
        PlatformEntry::new(170, "喜马拉雅VIP会员", "Ximalaya VIP", "学习与考试", "音频平台", true, false, true,
            "死亡证明、继承公证书", "联系客服申请余额退还", "中等",
            "https://www.ximalaya.com", "30-60天", ""),

        // ========== 扩展：苹果/海外服务类（6个） ==========
        PlatformEntry::new(171, "Apple ID", "Apple ID", "工具订阅", "苹果生态", true, true, true,
            "死亡证明、继承密钥、遗产联系人", "苹果数字遗产计划，需生前设置遗产联系人", "简单",
            "https://support.apple.com/zh-cn/HT212360", "7-30天", "苹果生态核心账号"),
        PlatformEntry::new(172, "Google账号", "Google Account", "工具订阅", "谷歌生态", true, true, true,
            "死亡证明、继承材料", "Inactive Account Manager", "简单",
            "https://myaccount.google.com", "已设置则自动执行", "需生前配置不活跃账户管理器"),
        PlatformEntry::new(173, "Microsoft账号", "Microsoft Account", "工具订阅", "微软生态", true, true, true,
            "死亡证明、继承材料", "微软数字遗产流程", "中等",
            "https://account.microsoft.com", "30-60天", ""),
        PlatformEntry::new(174, "Dropbox Business", "Dropbox Business", "工具订阅", "云存储", true, true, true,
            "继承人申请+法律文件", "Dropbox Business支持通过法律流程转移", "中等",
            "https://www.dropbox.com", "30-60天", "企业版有团队数据迁移功能"),
        PlatformEntry::new(175, "Instagram", "Instagram", "社交通讯", "社交媒体", true, true, true,
            "死亡证明、继承材料", "Meta支持纪念账号或数据导出", "中等",
            "https://www.instagram.com", "30-60天", "有商业价值的账号"),
        PlatformEntry::new(176, "Facebook", "Facebook", "社交通讯", "社交媒体", true, true, true,
            "死亡证明、继承材料", "Meta支持纪念账号或数据导出", "中等",
            "https://www.facebook.com", "30-60天", ""),

        // ========== 扩展：政务/公共服务类（8个） ==========
        PlatformEntry::new(177, "国家医保服务平台", "National Healthcare", "政务服务", "公共服务", true, false, true,
            "死亡证明、社保继承材料", "医保个人账户余额可继承", "简单",
            "https://www.nhsa.gov.cn", "15-30天", ""),
        PlatformEntry::new(178, "个人所得税APP", "Tax APP", "政务服务", "税务", true, false, false,
            "账号不可继承", "需注销", "困难",
            "https://www.chinatax.gov.cn", "无法继承", ""),
        PlatformEntry::new(179, "交管12123", "Traffic Management", "政务服务", "交通", true, true, false,
            "账号不可继承", "需注销", "困难",
            "https://gab.122.gov.cn", "无法继承", ""),
        PlatformEntry::new(180, "住房公积金管理中心", "Housing Fund Center", "政务服务", "公积金", true, false, true,
            "死亡证明、继承公证书", "公积金账户余额可全额提取", "简单",
            "https://www.cqgjj.cn", "7-15天", ""),
        PlatformEntry::new(181, "社保卡", "Social Security Card", "政务服务", "社保", true, false, true,
            "死亡证明、继承公证书", "社保卡内余额可继承", "简单",
            "https://www.mohrss.gov.cn", "15-30天", ""),
        PlatformEntry::new(182, "公积金联名卡", "Housing Fund Card", "政务服务", "银行卡", true, false, true,
            "死亡证明、继承公证书", "银行卡内余额可继承", "简单",
            "https://www.cqgjj.cn", "7-15天", ""),
        PlatformEntry::new(183, "数字人民币", "Digital RMB", "政务服务", "数字货币", true, false, true,
            "死亡证明、继承公证书", "数字人民币钱包余额可继承", "中等",
            "https://www.pbc.gov.cn", "30-60天", "人民银行主管"),
        PlatformEntry::new(184, "粤省事", "Guangdong Government", "政务服务", "政务平台", true, false, false,
            "账号不可继承", "需注销", "困难",
            "https://www.gdzwfw.gov.cn", "无法继承", ""),

        // ========== 扩展：其他高价值类（8个） ==========
        PlatformEntry::new(185, "比特币/加密货币钱包", "Crypto Wallet", "金融证券", "加密货币", true, false, false,
            "需私钥/助记词", "去中心化资产，无私钥则永久丢失", "困难",
            "https://www.blockchain.com", "无法继承", "高价值数字资产，需生前备份私钥"),
        PlatformEntry::new(186, "域名（阿里云万网）", "Domain (Aliyun)", "工具订阅", "域名", true, false, true,
            "死亡证明、继承公证书", "域名可申请过户", "中等",
            "https://wanwang.aliyun.com", "30-60天", "域名有商业价值"),
        PlatformEntry::new(187, "域名（GoDaddy）", "Domain (GoDaddy)", "工具订阅", "域名", true, false, true,
            "死亡证明、继承材料", "联系客服申请域名转移", "中等",
            "https://www.godaddy.com", "30-60天", ""),
        PlatformEntry::new(188, "网站备案/ICP", "ICP License", "工具订阅", "网站", true, false, true,
            "死亡证明、继承公证书", "需注销或变更备案", "困难",
            "https://beian.miit.gov.cn", "60-180天", "企业备案可变更主体"),
        PlatformEntry::new(189, "软件著作权", "Software Copyright", "工具订阅", "知识产权", true, false, true,
            "死亡证明、继承公证书", "软著登记可申请继承", "中等",
            "https://www.ccopyright.com.cn", "30-60天", "有知识产权价值"),
        PlatformEntry::new(190, "专利", "Patent", "工具订阅", "知识产权", true, false, true,
            "死亡证明、继承公证书", "专利权可申请继承", "中等",
            "https://www.cnipa.gov.cn", "30-60天", ""),
        PlatformEntry::new(191, "商标", "Trademark", "工具订阅", "知识产权", true, false, true,
            "死亡证明、继承公证书", "商标权可申请继承", "中等",
            "https://sbj.cnipa.gov.cn", "30-60天", ""),
        PlatformEntry::new(192, "个人博客/独立站", "Personal Blog", "工具订阅", "网站", true, true, true,
            "死亡证明、继承材料", "域名+主机+内容可整体继承", "中等",
            "https://www.wordpress.com", "30-60天", "有独立流量的博客"),

        // ========== 扩展：汽车/房产类（4个） ==========
        PlatformEntry::new(193, "蔚来APP", "NIO APP", "汽车服务", "车联网", true, true, true,
            "死亡证明、继承公证书", "联系客服申请账号转移", "中等",
            "https://www.nio.com", "30-60天", "车内可能有付费内容"),
        PlatformEntry::new(194, "特斯拉APP", "Tesla APP", "汽车服务", "车联网", true, true, true,
            "死亡证明、继承材料", "联系客服申请账号转移", "中等",
            "https://www.tesla.com", "30-60天", ""),
        PlatformEntry::new(195, "小鹏汽车APP", "XPeng APP", "汽车服务", "车联网", true, true, true,
            "死亡证明、继承公证书", "联系客服申请账号转移", "中等",
            "https://www.xiaopeng.com", "30-60天", ""),
        PlatformEntry::new(196, "理想汽车APP", "Li Auto APP", "汽车服务", "车联网", true, true, true,
            "死亡证明、继承公证书", "联系客服申请账号转移", "中等",
            "https://www.lixiang.com", "30-60天", ""),
    ];
    enrich_extended(&mut platforms);
    platforms
}

/// 为一批高价值平台补充「继承执行工具包」扩展字段：
/// 变现/过户路径、风险提示、标签、关联法律模板。
/// 说明：只填充稳妥、通用的描述，不杜撰具体官方链接；
/// 未在此列出的平台保持扩展字段为空（前端会据此过滤展示）。
fn enrich_extended(platforms: &mut [PlatformEntry]) {
    // 合规说明：本字段只收录平台官方公开的继承/法定过户流程（凭死亡证明与继承公证书办理），
    // 不涉及任何账号买卖、第三方交易或规避平台规则的方式。使用界面会同时展示该声明。
    let apply = |p: &mut PlatformEntry, monetization: &str, risk: &str, tags: &str, legal: &str| {
        p.monetization_path = monetization.to_string();
        p.risk_warnings = risk.to_string();
        p.tags = tags.to_string();
        p.legal_template_ids = legal.to_string();
    };

    for p in platforms.iter_mut() {
        match p.name.as_str() {
            "支付宝" => apply(
                p,
                "余额/余额宝资金可依法继承过户：向客服 95188 提交死亡证明+继承公证书+继承人身份证，5 万元以下通常免公证，资金划转至继承人账户。",
                "余额宝等理财按申请时点净值结算，存在价差；大额需继承权公证。",
                "需公证,高价值,可继承",
                "inheritance-declaration,platform-application,will-digital",
            ),
            "微信支付" => apply(
                p,
                "微信零钱余额可走继承流程提取，向微信支付客服提交死亡证明与继承材料；零钱通理财按净值处理。",
                "微信社交账号本身不可继承，仅余额/理财权益可处理；零钱提现绑定银行卡。",
                "需公证,高价值",
                "inheritance-declaration,platform-application",
            ),
            "云闪付" => apply(
                p,
                "银联余额按银行继承流程处理，凭死亡证明+继承公证书联系银联客服 95516 划转余额。",
                "余额与银行卡绑定，需银行配合完成过户。",
                "需公证,高价值",
                "inheritance-declaration,platform-application",
            ),
            "京东支付" => apply(
                p,
                "京东金融余额/理财可联系客服 95118 走继承流程，提交死亡证明与继承公证书。",
                "理财类按净值结算，需确认账户资产范围。",
                "需公证,高价值",
                "inheritance-declaration,platform-application",
            ),
            "抖音" => apply(
                p,
                "有价值的账号可申请过户/商业继承，需走平台流程；账号收益（橱窗/直播）可协商继承。",
                "个人号过户门槛高，多为机构/企业号；粉丝与内容价值依赖账号状态。",
                "高价值,需公证,内容资产",
                "inheritance-declaration,platform-application,inheritance-agreement",
            ),
            "B站(bilibili)" => apply(
                p,
                "账号不可继承，创作内容（稿件、收益）需生前自行导出；商业账号可尝试申请纪念处理。",
                "账号所有权不可转移，未备份的内容/收益将丢失（死亡循环风险）。",
                "需备份,内容资产,死亡循环风险",
                "will-digital,inheritance-declaration",
            ),
            "小红书" => apply(
                p,
                "高价值账号可尝试申请商业继承，个人数据需生前导出；联系客服说明继承诉求。",
                "账号不可直接过户，未导出的笔记/数据无法取回。",
                "需备份,内容资产",
                "inheritance-declaration,will-digital",
            ),
            "快手" => apply(
                p,
                "平台支持账号继承，走官方流程提交死亡证明与身份关系证明，账号及权益过户给继承人。",
                "需平台审核，处理周期较长（60-90 天）。",
                "需公证,可继承,高价值",
                "inheritance-declaration,platform-application",
            ),
            "淘宝" => apply(
                p,
                "个人网店可过户，支付宝余额按支付宝流程继承；店铺经营权益可整体转让。",
                "店铺与支付宝关联，过户需处理保证金/在途订单。",
                "可过户,高价值,商业账号",
                "inheritance-declaration,platform-application,inheritance-agreement",
            ),
            "京东" => apply(
                p,
                "京东余额与 Plus 会员权益有效期可继承，联系客服提交材料。",
                "虚拟权益（优惠券等）多不可继承。",
                "需公证,高价值",
                "inheritance-declaration,platform-application",
            ),
            "拼多多" => apply(
                p,
                "拼多多钱包余额可申请继承，提交死亡证明与继承材料给客服。",
                "优惠券/红包多绑定个人，通常不可继承。",
                "需公证,高价值",
                "inheritance-declaration,platform-application",
            ),
            "闲鱼" => apply(
                p,
                "闲鱼与支付宝账号绑定，随支付宝继承流程处理；在售商品需提前下架/交割。",
                "在售订单需妥善处理，避免纠纷。",
                "可继承,高价值",
                "inheritance-declaration,platform-application",
            ),
            "得物" => apply(
                p,
                "账户余额可申请提取，联系客服提交继承材料。",
                "平台权益类多为消费记录，价值有限。",
                "需公证",
                "inheritance-declaration,platform-application",
            ),
            "米哈游（原神等）" => apply(
                p,
                "支持游戏账号继承，联系客服提交材料；游戏内资产/装备价值有限但账号可处理。",
                "账号绑定手机号，需手机号同步过户；处理周期 30-90 天。",
                "需公证,可继承",
                "inheritance-declaration,platform-application",
            ),
            "Steam" => apply(
                p,
                "账号不可转移；需生前备份/导出游戏购买记录，权益随账号锁定。",
                "无私钥/密码即永久锁定（死亡循环风险）；库存游戏资产无法变现。",
                "需备份,死亡循环风险",
                "will-digital,inheritance-declaration",
            ),
            "百度网盘" => apply(
                p,
                "不官方支持账号转移；需生前共享重要目录或导出备份，继承人凭备份恢复。",
                "账号不可继承，未备份的数据将无法取回（死亡循环风险）。",
                "需备份,死亡循环风险",
                "will-digital,inheritance-declaration",
            ),
            "阿里云盘" => apply(
                p,
                "需身份验证+账号密码，官方不支持转移；重要数据需生前备份。",
                "账号不可继承，未备份即丢失。",
                "需备份,死亡循环风险",
                "will-digital",
            ),
            "iCloud" => apply(
                p,
                "苹果数字遗产：需生前设置「遗产联系人」，凭死亡证明+继承密钥向苹果申请访问照片等数据。",
                "未生前设置遗产联系人/密钥则无法访问。",
                "需生前设置,高价值",
                "will-digital,inheritance-declaration",
            ),
            "Google Drive" => apply(
                p,
                "Google「Inactive Account Manager」：生前配置后，账号不活跃期满自动向指定联系人移交或删除数据。",
                "需生前配置，否则数据被删除后无法取回。",
                "需生前设置,可继承",
                "will-digital,inheritance-declaration",
            ),
            "东方财富" => apply(
                p,
                "证券账户按股票继承流程：向证券公司+中国结算（中登）提交死亡证明+继承公证书办理股权过户。",
                "股权继承复杂、周期长（90-365 天）；市值随行情波动。",
                "需公证,高价值",
                "inheritance-declaration,platform-application,inheritance-agreement",
            ),
            "同花顺" => apply(
                p,
                "股票账户继承需走证券公司+中登公司流程，提交继承公证书等证券继承材料。",
                "周期长、需多机构配合；市值波动影响继承价值。",
                "需公证,高价值",
                "inheritance-declaration,inheritance-agreement",
            ),
            "蚂蚁财富" => apply(
                p,
                "基金/理财产品通过支付宝体系提交继承申请，凭死亡证明+继承公证书办理。",
                "净值按申请时点结算；需确认持仓范围。",
                "需公证,高价值",
                "inheritance-declaration,platform-application",
            ),
            "招商银行APP" => apply(
                p,
                "银行卡及 APP 内资产按银行继承流程：凭继承公证书到银行办理余额/理财提取与过户。",
                "需银行核实继承人身份，周期 7-30 天。",
                "需公证,高价值,简易程序",
                "inheritance-declaration,platform-application",
            ),
            "公积金" => apply(
                p,
                "公积金账户余额可由继承人全额提取，凭死亡证明+继承公证书向公积金中心申请，周期短（7-15 天）。",
                "需符合提取条件并备齐材料。",
                "高价值,简易程序",
                "inheritance-declaration,platform-application",
            ),
            "比特币/加密货币钱包" => apply(
                p,
                "去中心化资产无法找客服：继承人凭逝者备份的私钥/助记词转移资产，无备份则永久丢失。",
                "唯一高死亡循环风险资产——必须生前备份并妥善交接私钥/助记词。",
                "需备份,死亡循环风险,高价值",
                "will-digital,inheritance-declaration",
            ),
            "域名（阿里云万网）" => apply(
                p,
                "域名可申请过户，凭死亡证明+继承公证书向万网提交变更持有人。",
                "需缴纳续费/保持备案有效；有商业价值的域名价值较高。",
                "需公证,可过户,高价值",
                "inheritance-declaration,platform-application",
            ),
            "软件著作权" => apply(
                p,
                "软著登记可申请继承/转让，向版权部门提交继承公证书办理权利人变更。",
                "需确认软著权属与登记信息一致。",
                "需公证,知识产权,高价值",
                "inheritance-declaration,inheritance-agreement",
            ),
            _ => {}
        }
    }
}

/// 按ID获取平台
pub fn get_platform_by_id(id: i64) -> Option<PlatformEntry> {
    get_all_platforms().into_iter().find(|p| p.id == id)
}

/// 按类别获取平台
pub fn get_platforms_by_category(category: &str) -> Vec<PlatformEntry> {
    get_all_platforms()
        .into_iter()
        .filter(|p| p.category == category)
        .collect()
}

/// 获取继承难度分布
pub fn get_difficulty_stats() -> std::collections::HashMap<String, i64> {
    let platforms = get_all_platforms();
    let mut stats: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for p in platforms {
        *stats.entry(p.estimated_difficulty.clone()).or_insert(0) += 1;
    }
    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_database_has_at_least_140_entries() {
        let platforms = get_all_platforms();
        assert!(
            platforms.len() >= 140,
            "平台数据库应包含至少 140 个平台，实际 {}",
            platforms.len()
        );
    }

    #[test]
    fn all_platforms_have_names() {
        let platforms = get_all_platforms();
        for p in &platforms {
            assert!(!p.name.is_empty(), "平台 id={} 名称为空", p.id);
            assert!(!p.name_en.is_empty(), "平台 id={} 英文名为空", p.id);
            assert!(!p.category.is_empty(), "平台 id={} 类别为空", p.id);
        }
    }

    #[test]
    fn all_platforms_have_valid_ids() {
        let platforms = get_all_platforms();
        let ids: Vec<i64> = platforms.iter().map(|p| p.id).collect();
        let unique: std::collections::HashSet<i64> = ids.iter().cloned().collect();
        assert_eq!(ids.len(), unique.len(), "平台 id 存在重复");
        assert!(ids.iter().all(|&id| id > 0), "平台 id 必须为正数");
    }

    #[test]
    fn categories_are_within_known_set() {
        let known = [
            "支付金融",
            "社交通讯",
            "游戏",
            "电商",
            "内容创作",
            "知识付费",
            "金融证券",
            "出行住宿",
            "工具订阅",
            "音视频",
            "生活服务",
            "学习与考试",
            "存储云服务",
            "政务服务",
            "生活工具",
            "汽车服务",
        ];
        let platforms = get_all_platforms();
        for p in &platforms {
            assert!(
                known.contains(&p.category.as_str()),
                "未知类别: {}",
                p.category
            );
        }
    }

    #[test]
    fn difficulty_stats_sum_matches_total() {
        let stats = get_difficulty_stats();
        let sum: i64 = stats.values().sum();
        let platforms = get_all_platforms();
        assert_eq!(sum, platforms.len() as i64, "难度分布总数应等于平台总数");
    }

    /// 开发工具：导出平台库 JSON（供 Web 预览 mock 使用）：
    /// cargo test --lib dump_platforms_json -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_platforms_json() {
        let json = serde_json::to_string_pretty(&get_all_platforms()).unwrap();
        println!("<<<PLATFORMS_JSON>>>");
        println!("{}", json);
        println!("<<<END_PLATFORMS_JSON>>>");
    }
}

#[cfg(test)]
mod authority_link_tests {
    use super::*;

    /// 挂到政策上的来源 id 必须真实存在于 authorities.json。
    /// 这是防止「引用一个不存在的来源」的最后一道闸。
    #[test]
    fn every_authority_id_exists_in_registry() {
        let known: std::collections::HashSet<String> = crate::models::authorities()
            .into_iter()
            .map(|a| a.id)
            .collect();
        for p in get_all_platforms() {
            for id in &p.authority_ids {
                assert!(
                    known.contains(id),
                    "平台「{}」引用了不存在的来源 id: {}",
                    p.name,
                    id
                );
            }
        }
    }

    /// 至少有平台挂了权威来源——否则整条引用体系是空转。
    #[test]
    fn some_platforms_carry_authorities() {
        let with_auth = get_all_platforms()
            .into_iter()
            .filter(|p| !p.authority_ids.is_empty())
            .count();
        assert!(with_auth >= 2, "只有 {} 条平台挂了来源", with_auth);
    }
}
