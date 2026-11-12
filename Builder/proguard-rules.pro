-keepclasseswithmembernames class com.code.builder.Builder {
    native <methods>;
}
-keep class com.code.builder.Builder { *; }

-keep class com.code.builder.BuilderActivity { *; }
-keep class com.code.builder.BuilderApplication { *; }

-keepattributes SourceFile,LineNumberTable
-renamesourcefileattribute SourceFile
