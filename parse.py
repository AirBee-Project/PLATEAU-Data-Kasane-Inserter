import json
import re
from html.parser import HTMLParser

class PlateauParser(HTMLParser):
    def __init__(self):
        super().__init__()
        self.regions = []
        self.current_region = None
        self.current_pref = None
        
        # State tracking flags
        self.in_accordion_head = False
        self.in_head_p = False
        self.in_pref_span = False
        self.in_region_span = False
        self.in_a = False
        
        self.current_a_href = None
        self.current_a_title = None
        self.city_text_buffer = ""
        self.a_text_buffer = ""

    def handle_starttag(self, tag, attrs):
        attrs_dict = dict(attrs)
        cls = attrs_dict.get("class", "")
        
        if tag == "div" and "p-accordion__head" in cls:
            self.in_accordion_head = True
        elif tag == "p" and self.in_accordion_head:
            self.in_head_p = True
        elif tag == "span" and "pref" in cls:
            self.in_pref_span = True
        elif tag == "span" and "region" in cls:
            self.in_region_span = True
            self.current_datasets = []
            self.city_text_buffer = ""
        elif tag == "a" and self.in_region_span:
            self.in_a = True
            self.current_a_href = attrs_dict.get("href", "")
            self.current_a_title = attrs_dict.get("title", "")
            self.a_text_buffer = ""

    def handle_endtag(self, tag):
        if tag == "div":
            if self.in_accordion_head:
                self.in_accordion_head = False
        elif tag == "p":
            if self.in_head_p:
                self.in_head_p = False
        elif tag == "span":
            if self.in_pref_span:
                self.in_pref_span = False
            elif self.in_region_span:
                self.in_region_span = False
                raw_city_name = self.city_text_buffer.strip()
                # Clean up all colons, commas, and spaces
                city_name = re.sub(r'[：,，\s]+', '', raw_city_name)
                if city_name and not (city_name.startswith("＜") and city_name.endswith("＞")):
                    if self.current_datasets:
                        self.add_city_to_current(city_name, self.current_datasets)
                self.current_datasets = []
        elif tag == "a" and self.in_a:
            self.in_a = False
            year_text = self.a_text_buffer.strip()
            
            # Parse year and optional note
            year_match = re.match(r"(\d+)(?:\s*[（(](.*?)[）)])?", year_text)
            if year_match:
                year = int(year_match.group(1))
                note = year_match.group(2) if year_match.group(2) else None
            else:
                year = 0
                note = year_text if year_text else None
                
            self.current_datasets.append({
                "year": year,
                "url": self.current_a_href,
                "title": self.current_a_title,
                "note": note
            })
            
    def handle_data(self, data):
        if self.in_head_p:
            region_name = data.strip()
            if region_name and "すべて閉じる" not in region_name:
                self.current_region = {
                    "name": region_name,
                    "prefectures": []
                }
                self.regions.append(self.current_region)
                self.current_pref = None
            self.in_head_p = False
            self.in_accordion_head = False
        elif self.in_pref_span:
            pref_name = data.strip()
            if pref_name:
                if self.current_region:
                    self.current_pref = {
                        "name": pref_name,
                        "cities": []
                    }
                    self.current_region["prefectures"].append(self.current_pref)
                self.in_pref_span = False
        elif self.in_region_span:
            if self.in_a:
                self.a_text_buffer += data
            else:
                self.city_text_buffer += data

    def add_city_to_current(self, city_name, datasets):
        if not self.current_region:
            return
            
        if not self.current_pref:
            pref_name = self.current_region["name"]
            existing = [p for p in self.current_region["prefectures"] if p["name"] == pref_name]
            if existing:
                self.current_pref = existing[0]
            else:
                self.current_pref = {
                    "name": pref_name,
                    "cities": []
                }
                self.current_region["prefectures"].append(self.current_pref)
                
        self.current_pref["cities"].append({
            "name": city_name,
            "datasets": datasets
        })

# Load and parse html
with open("portal_site.html", "r", encoding="utf-8") as f:
    html_content = f.read()

parser = PlateauParser()
parser.feed(html_content)

# Save the parsed data to a json file
with open("parsed_data.json", "w", encoding="utf-8") as f:
    json.dump(parser.regions, f, ensure_ascii=False, indent=2)

print("Parsed successfully!")
