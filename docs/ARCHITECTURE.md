# Architecture

## System Overview

```mermaid
graph LR
    subgraph "Client Layer"
        C[CLI]
        A[REST API]
        M[MCP Server]
        W[WASM]
    end

    subgraph "Processing Layer"
        IC[Image Core]
        PC[PDF Core]
        AC[AI Core]
    end

    subgraph "Storage Layer"
        FS[Local Filesystem]
        TM[Temp Directory]
        MD[Model Cache]
    end

    C --> IC
    C --> PC
    C --> AC
    A --> IC
    A --> PC
    A --> AC
    M --> IC
    M --> PC
    M --> AC
    W --> IC

    IC --> FS
    IC --> TM
    PC --> FS
    PC --> TM
    AC --> FS
    AC --> MD
```

## Image Processing Pipeline

```mermaid
flowchart TD
    Start([Start]) --> Validate{Validate Input}
    Validate -->|Invalid| Error[Return Error]
    Validate -->|Valid| Load[Load Image]
    
    Load --> Detect{Detect Format}
    Detect -->|JPEG| JPEG[Process JPEG]
    Detect -->|PNG| PNG[Process PNG]
    Detect -->|WebP| WEBP[Process WebP]
    Detect -->|HEIC| HEIC[Process HEIC]
    Detect -->|Other| OTHER[Process Generic]
    
    JPEG --> Apply{Apply Operation}
    PNG --> Apply
    WEBP --> Apply
    HEIC --> Apply
    OTHER --> Apply
    
    Apply -->|Compress| Compress[Compress Image]
    Apply -->|Convert| Convert[Convert Format]
    Apply -->|Resize| Resize[Resize Image]
    Apply -->|Crop| Crop[Crop Image]
    Apply -->|Filter| Filter[Apply Filter]
    Apply -->|Watermark| Watermark[Add Watermark]
    
    Compress --> Save[Save Output]
    Convert --> Save
    Resize --> Save
    Crop --> Save
    Filter --> Save
    Watermark --> Save
    
    Save --> Stats[Calculate Stats]
    Stats --> End([Return Result])
```

## PDF Processing Pipeline

```mermaid
flowchart TD
    Start([Start]) --> Validate{Validate PDF}
    Validate -->|Invalid| Error[Return Error]
    Validate -->|Valid| Load[Load PDF with lopdf]
    
    Load --> Type{Operation Type}
    
    Type -->|Merge| Merge[Merge Documents]
    Type -->|Split| Split[Split Document]
    Type -->|Compress| Compress[Compress Document]
    Type -->|OCR| OCR[Extract Text]
    Type -->|ToImage| ToImage[Render Pages]
    
    Merge --> Iterate[Iterate Source PDFs]
    Iterate --> CopyPages[Copy Pages]
    CopyPages --> Combine[Combine Documents]
    Combine --> SavePDF[Save Output PDF]
    
    Split --> Select[Select Pages]
    Select --> Extract[Extract Pages]
    Extract --> SavePDF
    
    Compress --> Optimize[Optimize Objects]
    Optimize --> RemoveDup[Remove Duplicates]
    RemoveDup --> Deflate[Deflate Streams]
    Deflate --> SavePDF
    
    OCR --> Render[Render to Image]
    Render --> Tesseract[Tesseract OCR]
    Tesseract --> TextLayer[Add Text Layer]
    TextLayer --> SavePDF
    
    ToImage --> RenderPage[Render Each Page]
    RenderPage --> SaveImage[Save as Image]
    
    SavePDF --> End([Return Result])
    SaveImage --> End
```

## AI Processing Pipeline

```mermaid
flowchart TD
    Start([Start]) --> Collect[Collect Input Files]
    
    Collect --> Strategy{Organization Strategy}
    
    Strategy -->|ByDate| DateSort[Sort by Date]
    Strategy -->|BySubject| SubjectClass[Classify Subject]
    Strategy -->|ByLocation| LocationSort[Sort by GPS]
    
    DateSort --> CreateFolders[Create Date Folders]
    SubjectClass --> AIModel[Run AI Model]
    LocationSort --> GPSLookup[Lookup Location]
    
    AIModel --> CLIP[CLIP Classification]
    CLIP --> Categories[Assign Categories]
    Categories --> CreateFolders
    
    GPSLookup --> CreateFolders
    
    CreateFolders --> Move[Move Files]
    Move --> Rename[Optional Rename]
    Rename --> End([Return Results])
    
    subgraph "AI Models"
        CLIP
        BLIP[BLIP Captioning]
        OCR[OCR Engine]
    end
    
    subgraph "Duplicate Detection"
        Hash[Perceptual Hash]
        Compare[Compare Hashes]
        Group[Group Duplicates]
    end
```

## Data Flow Diagram

```mermaid
sequenceDiagram
    participant U as User
    participant CLI as CLI
    participant Core as Core
    participant Img as Image Processor
    participant PDF as PDF Processor
    participant AI as AI Processor
    participant FS as FileSystem

    Note over U,FS: Image Compression Flow
    U->>CLI: rtools image compress --input photo.jpg
    CLI->>Core: Validate request
    Core->>Img: CompressProcessor.process()
    Img->>FS: Read photo.jpg
    FS-->>Img: File data
    Img->>Img: Apply compression
    Img->>FS: Write photo_compressed.jpg
    FS-->>Img: Write success
    Img-->>Core: ProcessResult
    Core-->>CLI: Output path + stats
    CLI-->>U: "Compressed: photo_compressed.jpg (45% smaller)"

    Note over U,FS: PDF Merge Flow
    U->>CLI: rtools pdf merge --input a.pdf b.pdf --output merged.pdf
    CLI->>Core: Validate request
    Core->>PDF: PdfMergeProcessor.process()
    PDF->>FS: Read a.pdf
    PDF->>FS: Read b.pdf
    FS-->>PDF: PDF data
    PDF->>PDF: Merge pages
    PDF->>FS: Write merged.pdf
    FS-->>PDF: Write success
    PDF-->>Core: ProcessResult
    Core-->>CLI: Output path
    CLI-->>U: "Merged: merged.pdf"

    Note over U,FS: AI Organize Flow
    U->>CLI: rtools ai organize --input ~/Photos
    CLI->>Core: Validate request
    Core->>AI: OrganizeProcessor.process()
    AI->>FS: List all images
    FS-->>AI: File list
    AI->>AI: Classify by date/subject
    AI->>FS: Create folders
    AI->>FS: Move files
    FS-->>AI: Move success
    AI-->>Core: List of organized files
    Core-->>CLI: Summary
    CLI-->>U: "Organized 150 photos into 12 folders"
```

## Error Handling Flow

```mermaid
flowchart TD
    Start([Operation]) --> Try[Execute Operation]
    
    Try -->|Success| Success[Return Success]
    
    Try -->|IO Error| IOError[Handle IO Error]
    Try -->|Format Error| FormatError[Handle Format Error]
    Try -->|Config Error| ConfigError[Handle Config Error]
    Try -->|Timeout| Timeout[Handle Timeout]
    
    IOError --> Log[Log Error]
    FormatError --> Log
    ConfigError --> Log
    Timeout --> Log
    
    Log --> Retry{Retry?}
    Retry -->|Yes| Try
    Retry -->|No| Return[Return Error]
    
    Success --> Stats[Collect Stats]
    Stats --> End([Done])
    Return --> End
```

## Configuration Flow

```mermaid
sequenceDiagram
    participant App as Application
    participant Config as Config Manager
    participant File as Config File
    participant Env as Environment

    App->>Config: Load configuration
    Config->>Env: Check environment variables
    Env-->>Config: Override values
    Config->>File: Check rtools.toml
    alt File exists
        File-->>Config: File values
    else File missing
        Config->>Config: Use defaults
    end
    Config->>Config: Merge all sources
    Config-->>App: AppConfig
```

## Module Dependencies

```mermaid
graph TD
    CLI[rtools-cli] --> Core[rtools-core]
    CLI --> Img[rtools-image]
    CLI --> PDF[rtools-pdf]
    CLI --> AI[rtools-ai]

    API[rtools-api] --> Core
    API --> Img
    API --> PDF
    API --> AI

    MCP[rtools-mcp] --> Core
    MCP --> Img
    MCP --> PDF
    MCP --> AI

    WASM[rtools-wasm] --> Core
    WASM --> Img

    Img --> Core
    PDF --> Core
    AI --> Core
    AI --> Img

    style Core fill:#e1f5fe
    style Img fill:#f3e5f5
    style PDF fill:#e8f5e9
    style AI fill:#fff3e0
    style CLI fill:#fce4ec
    style API fill:#e0f2f1
    style MCP fill:#f1f8e9
    style WASM fill:#fff8e1
```