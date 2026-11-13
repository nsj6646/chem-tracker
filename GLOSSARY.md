# Chemical Import Volume Tracker (Korea Branch)

A desktop database application used by the Korea branch office to manage confidential product chemical recipes (BOM), track import shipments between overseas branches and domestic importing companies, and aggregate annual chemical substance import volumes (`kg` and `ton`) by `CAS번호`.

## Language

### Parties & Products

**Domestic Importer** (`국내 수입사` / `수입회사`):
A company in South Korea that imports products from an Overseas Branch. Annual chemical import volume obligations are tracked per Domestic Importer.
_Avoid_: Customer, client, buyer

**Overseas Branch** (`해외지사` / `공급지사`):
An overseas branch or manufacturing affiliate that sells and supplies products to a Domestic Importer.
_Avoid_: Vendor, supplier, manufacturer

**Imported Product** (`수입제품`):
A commercial chemical mixture or product identified by an **Imported Product Code** (`수입제품코드`) and supplied by an Overseas Branch.
_Avoid_: Item, goods, SKU

**Product Recipe** (`제품 구성성분` / `레시피` / `BOM`):
The confidential list of **Chemical Substances** (referenced by `CAS번호`) and their **Weight Percentage** (`함유율 %`, whose total across a product must equal `100%`) contained in an **Imported Product**. Stored strictly inside the Korea branch's local database and never exposed directly to outside parties or the developer.
_Avoid_: Formula, ingredients

### Transactions & Calculations

**Import Shipment** (`수입실적` / `수입내역`):
A dated record (`수입일자`) of a specific **Imported Product** imported by a **Domestic Importer** from an **Overseas Branch** with a specific **Product Import Weight** (`제품 수입량`, entered in `kg` with decimal precision and displayed alongside `ton` = `kg / 1,000`).
_Avoid_: Order, transaction, invoice

**Substance Import Weight** (`화학물질 환산 중량`):
The calculated weight (in `kg` and `ton`) of a single **Chemical Substance** within an **Import Shipment**, computed via fixed-point/exact decimal arithmetic as $\text{Product Import Weight} \times (\text{Weight Percentage} / 100)$.

**Annual Substance Total** (`연간 화학물질 총 수입량`):
The aggregated sum (`SUM`, in `kg` and `ton`) of **Substance Import Weight** for each unique `CAS번호` imported by a **Domestic Importer** within a calendar year (`연간`).

### Chemical Substances

**Chemical Substance** (`화학물질`):
A chemical component managed directly in the Korea branch's database and identified by `CAS번호` (entered manually by the user or migrated from the customer's existing Access database, with no dependency on external regulatory master files).
