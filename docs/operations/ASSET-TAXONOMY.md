# Asset taxonomy

The asset register answers one question: **what does this person hold, and who should receive it?** The platform policy library answers a different question: **what does this platform require after death?** They are separate data sets and must not be merged.

## Three boundaries

- **Asset register**: accounts, files, devices, domains, subscriptions, financial interests, real estate, vehicles, intellectual property, business interests, virtual items, memberships, identity rights and physical objects.
- **Liability ledger** (reserved, not part of this release): mortgages, cards, consumer loans, guarantees, tax and unresolved disputes. A liability is not an asset and must never inflate the asset count or value.
- **Access materials** (reserved, not part of this release): password-manager locations, recovery codes, 2FA devices, API keys and key-holder instructions. They are custody support, not saleable assets, and must not be stored as plain notes.

The platform policy library stays independent. A policy row says how a platform handles inheritance; it does not mean the user owns that account.

## Entry groups

The asset form groups the `kind` field; legacy values remain valid and unchanged.

| Group | Kinds | Typical objects |
|---|---|---|
| Financial | `financial` | bank deposits, cards, funds, securities, insurance policies, pensions |
| Real estate & vehicles | `real_estate`, `vehicle` | homes, land, parking spaces, shops, cars, ships |
| Physical & collectibles | `physical` | jewellery, art, collectibles, safe-deposit contents |
| Intellectual property | `intellectual_property` | copyrights, patents, trademarks, source code, royalties |
| Business | `business` | equity, partnership share, licences, merchant accounts |
| Digital accounts | `account` | email, phone number, social, payment and cloud accounts |
| Virtual items | `virtual_item` | game inventory, skins, wallet balances, digital collectibles |
| Files, devices & domains | `file`, `device`, `domain` | documents, drives, servers, domain names, websites |
| Subscriptions & memberships | `subscription`, `membership` | SaaS plans, memberships, points, stored value |
| Identity & rights | `identity` | ID documents, qualifications, licences, benefit entitlements |
| Other | `other` | anything that does not fit above |

## Compatibility rules

- The original seven kinds (`account`, `file`, `device`, `domain`, `subscription`, `physical`, `other`) keep their identifiers and meaning.
- New kinds are additive. Existing rows need no migration; schema version stays v18.
- A kind belongs to exactly one group. The Web test `model.test.ts` fails if a kind is missing or duplicated.
- Adding a new kind requires updating the contract enum, the Rust allow-list, the Web union and the group map in the same change.
