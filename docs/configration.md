#Configuration
Eww’s configuration should be placed in ~/.config/eww/eww.xml and any scss styles you want to add should be put into ~/.config/eww/eww.scss.

for example-

```xml
<eww>
  <definitions>
    <def name="test">
      <layout orientation="v">
        {{foo}}
        <button onclick='notify-send "that hurt,..."'>
            click me if you dare :&lt;
          </button>
        <layout>
          {{ree}}
          <slider min="0" max="100" value="50" onchange="notify-send {}"/>
        </layout>
      </layout>
    </def>
  </definitions>

  <variables>
    <var name="foo">test</var>
  </variables>


  <windows>
    <window name="main_window">
      <size x="100" y="200" />
      <pos x="100" y="200" />
      <widget>
        <test ree="test" />
      </widget>
    </window>
  </windows>
</eww>
```
