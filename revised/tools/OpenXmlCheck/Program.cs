using DocumentFormat.OpenXml.Packaging;
using DocumentFormat.OpenXml.Validation;
if (args.Length != 1) { Console.Error.WriteLine("Usage: OpenXmlCheck <dataset folder>"); return 2; }
int count = 0, failures = 0;
foreach (var file in Directory.EnumerateFiles(args[0], "*", SearchOption.AllDirectories).Where(f => new[]{".docx",".xlsx",".pptx"}.Contains(Path.GetExtension(f).ToLowerInvariant()))) {
    try {
        using OpenXmlPackage package = Path.GetExtension(file).ToLowerInvariant() switch {
            ".docx" => WordprocessingDocument.Open(file, false),
            ".xlsx" => SpreadsheetDocument.Open(file, false),
            _ => PresentationDocument.Open(file, false)
        };
        var errors = new OpenXmlValidator().Validate(package).ToList();
        if (errors.Count != 0) { failures++; foreach(var e in errors) Console.Error.WriteLine($"{file}: {e.Path?.XPath}: {e.Description}"); }
        count++;
    } catch(Exception e) { failures++; Console.Error.WriteLine($"{file}: {e.Message}"); }
}
Console.WriteLine($"Open XML SDK: {count} packages checked, {failures} failed.");
return count == 0 || failures != 0 ? 1 : 0;
